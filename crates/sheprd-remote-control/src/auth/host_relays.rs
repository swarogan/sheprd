//! Both ends of carrying a phone's relay keys through another computer
//! (`control::host_relay`).
//!
//! On the near computer, [`HostRelays`] keeps the phones' requests in
//! memory for Sheprd to take, and what each came to for the phone to read.
//! A request lost to a restart is posted again by the phone.
//!
//! On the far computer, [`keep`] checks a request against the device key
//! the phone holds there and stores its relay keys.

use super::{DeviceRecord, Devices, Result, decode_fixed, devices::RelayKeys};
use crate::control::{
    DeviceId,
    host_relay::{self, Job, Outcome, Request, State},
};
use p256::ecdsa::{Signature, signature::Verifier};
use std::collections::HashMap;

/// Requests kept at once, across every phone.
const MAX_ASKS: usize = 256;
/// How long a request waits for Sheprd to take it: as long as its
/// signature lasts.
const QUEUED_TTL_MS: u64 = host_relay::MAX_AGE_MS;
/// How long Sheprd may take to carry a request it took, trying each host
/// over SSH, before the phone hears it failed: its own limit
/// (`host_relay::CARRY_MS`) and time to report.
pub(crate) const TAKEN_TTL_MS: u64 = host_relay::CARRY_MS + 2 * 60 * 1000;
/// How long an answer waits for the phone to read it.
const DONE_TTL_MS: u64 = 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    // keep-sorted start
    #[error("these relay keys are already being carried")]
    Busy,
    #[error("the relay keys or their signature are not valid")]
    Invalid,
    #[error("no request for those relay keys is being carried")]
    NotCarrying,
    #[error("the relay keys were signed too long ago, or in the future")]
    Stale,
    #[error("the phone sent newer relay keys since it signed these")]
    Superseded,
    #[error("too many relay keys are being carried; try again shortly")]
    TooMany,
    // keep-sorted end
}

#[derive(Debug)]
struct Ask {
    request: Request,
    host: Option<crate::control::hosts::HostId>,
    stage: Stage,
    since_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum Stage {
    Queued,
    Taken,
    Done(Outcome),
}

/// The near computer's requests, by the phone as it knows it and the far
/// computer's fingerprint.
#[derive(Debug, Default)]
pub(crate) struct HostRelays {
    asks: HashMap<(DeviceId, [u8; 32]), Ask>,
}

impl HostRelays {
    /// The phone `device` asks for `request` to be carried; answers where
    /// it stands. Posting the same request again only reads it, so the
    /// phone polls by posting.
    pub(crate) fn ask(
        &mut self,
        device: &DeviceRecord,
        request: Request,
        now_ms: u64,
    ) -> Result<State> {
        self.refresh(now_ms);
        let fingerprint = host_relay::fingerprint_bytes(&request.fingerprint)
            .filter(|_| request.valid())
            .ok_or(Error::Invalid)?;
        let key = (device.id.clone(), fingerprint);
        match self.asks.get(&key) {
            Some(ask) if ask.request == request => return Ok(state(&ask.stage)),
            Some(Ask {
                stage: Stage::Taken,
                ..
            }) => return Err(Error::Busy.into()),
            Some(_) => {}
            None if self.asks.len() >= MAX_ASKS => return Err(Error::TooMany.into()),
            None => {}
        }
        // A phone paired with the far computer through this one names the
        // host to try first.
        let host = device
            .links
            .iter()
            .find(|link| {
                link.paired.as_ref().is_some_and(|paired| {
                    host_relay::fingerprint_bytes(&paired.fingerprint) == Some(fingerprint)
                })
            })
            .map(|link| link.host.clone());
        self.asks.insert(
            key,
            Ask {
                request,
                host,
                stage: Stage::Queued,
                since_ms: now_ms,
            },
        );
        Ok(State::Pending)
    }

    /// Hands Sheprd the requests it has not taken yet.
    pub(crate) fn take(&mut self, now_ms: u64) -> Vec<Job> {
        self.refresh(now_ms);
        self.asks
            .iter_mut()
            .filter(|(_, ask)| ask.stage == Stage::Queued)
            .map(|((device, _), ask)| {
                ask.stage = Stage::Taken;
                ask.since_ms = now_ms;
                Job {
                    device: device.clone(),
                    host: ask.host.clone(),
                    request: ask.request.clone(),
                }
            })
            .collect()
    }

    /// Records what carrying `request` came to. Only the request Sheprd
    /// took takes it: one the phone replaced since keeps waiting.
    pub(crate) fn finish(
        &mut self,
        device: &DeviceId,
        request: &Request,
        outcome: Outcome,
        now_ms: u64,
    ) -> Result<()> {
        self.refresh(now_ms);
        // A claim the far computer did not sign is no answer.
        let claims = matches!(outcome, Outcome::Relayed { .. } | Outcome::Kept { .. });
        if !outcome.valid() || (claims && !outcome.proven(request)) {
            return Err(Error::Invalid.into());
        }
        let fingerprint =
            host_relay::fingerprint_bytes(&request.fingerprint).ok_or(Error::Invalid)?;
        let ask = self
            .asks
            .get_mut(&(device.clone(), fingerprint))
            .filter(|ask| ask.stage == Stage::Taken && &ask.request == request)
            .ok_or(Error::NotCarrying)?;
        let outcome = match outcome {
            Outcome::Failed { reason } => Outcome::Failed {
                reason: reason.chars().take(host_relay::MAX_REASON).collect(),
            },
            outcome => outcome,
        };
        ask.stage = Stage::Done(outcome);
        ask.since_ms = now_ms;
        Ok(())
    }

    /// A device revoked here: its requests end.
    pub(crate) fn forget(&mut self, device: &DeviceId) {
        self.asks.retain(|(id, _), _| id != device);
    }

    /// Fails requests Sheprd took and never finished, and drops answers
    /// nobody read and requests nobody took.
    fn refresh(&mut self, now_ms: u64) {
        self.asks.retain(|_, ask| {
            let age = now_ms.saturating_sub(ask.since_ms);
            match ask.stage {
                Stage::Queued => age < QUEUED_TTL_MS,
                Stage::Taken => true,
                Stage::Done(_) => age < DONE_TTL_MS,
            }
        });
        for ask in self.asks.values_mut() {
            if ask.stage == Stage::Taken && now_ms.saturating_sub(ask.since_ms) >= TAKEN_TTL_MS {
                ask.stage = Stage::Done(Outcome::Failed {
                    reason: "Sheprd did not finish carrying the relay keys".to_owned(),
                });
                ask.since_ms = now_ms;
            }
        }
    }
}

fn state(stage: &Stage) -> State {
    match stage {
        Stage::Queued | Stage::Taken => State::Pending,
        Stage::Done(outcome) => State::Done(outcome.clone()),
    }
}

/// On the far computer, whose CA has `fingerprint`: keeps the relay keys
/// of `request` when the phone signed them with its own device key here.
/// Answers whether the keys were kept, or that `request` names another
/// computer. Blocking when it keeps them.
pub(crate) fn keep(
    devices: &mut Devices,
    request: &Request,
    fingerprint: &[u8; 32],
    now_ms: u64,
) -> Result<bool> {
    let message = request.message().ok_or(Error::Invalid)?;
    if host_relay::fingerprint_bytes(&request.fingerprint).as_ref() != Some(fingerprint) {
        return Ok(false);
    }
    if !request.fresh(now_ms) {
        return Err(Error::Stale.into());
    }
    let keys = RelayKeys {
        dial: request.keys.dial.clone(),
        tunnel: request.keys.tunnel.clone(),
    };
    if keys.dial_key().is_none() || keys.tunnel_key().is_none() {
        return Err(Error::Invalid.into());
    }
    let record = devices
        .get(&request.device)
        .ok_or(super::Error::UnknownDevice)?;
    let signature: [u8; 64] = decode_fixed(&request.signature).map_err(|_| Error::Invalid)?;
    let signature = Signature::from_slice(&signature).map_err(|_| Error::Invalid)?;
    record
        .public_key
        .verifying_key()?
        .verify(&message, &signature)
        .map_err(|_| Error::Invalid)?;
    if record.relay.as_ref() == Some(&keys) {
        return Ok(true);
    }
    // Keys the phone sent since, directly or carried, win over these,
    // which may have spent minutes finding this computer.
    if record.relay_ms.is_some_and(|set| request.signed_ms <= set) {
        return Err(Error::Superseded.into());
    }
    if !devices.set_relay(&request.device, keys, now_ms)? {
        return Err(super::Error::UnknownDevice);
    }
    Ok(true)
}
