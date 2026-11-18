//! A T3 Code thread, followed live and served on the attach wire, read-only.
//!
//! T3 Code runs the thread's agent; this program only reads the thread's
//! stream (`sheprd_t3`) and shows it to Sheprd's chat views through the same
//! [`Mirror`] that serves existing CLI transcripts. Every command a client
//! sends is refused. One worker thread owns the T3 connection and hands the
//! main loop a fresh transcript after each chunk; when the connection ends,
//! the chat says why and keeps showing what it had.

use std::{
    io::{Read, Write},
    thread,
};

use anyhow::Context as _;
use crossbeam_channel::{bounded, select};
use serde_json::json;
use sheprd_chat_protocol::{EndpointId, Status, Transcript};
use sheprd_t3::{
    connection::Connection, discover, frame::ServerFrame, http::Server, thread::Thread,
};

use crate::{cli::T3Options, mirror::Mirror, session::Input};

/// What one frame of the thread's stream does to the chat.
#[derive(Debug, PartialEq)]
pub(crate) enum Step {
    /// The thread changed; this is its transcript now.
    Changed(Box<Transcript>),
    /// Nothing the chat shows changed.
    Same,
    /// The stream ended, for this reason.
    Ended(String),
}

/// The thread a stream follows, and what it last looked like.
pub(crate) struct Follower {
    request: String,
    thread: Thread,
}

impl Follower {
    pub(crate) fn new(request: String) -> Self {
        Self {
            request,
            thread: Thread::default(),
        }
    }

    /// Whether the thread's snapshot has arrived.
    pub(crate) fn ready(&self) -> bool {
        self.thread.sequence.is_some()
    }

    pub(crate) fn frame(&mut self, frame: ServerFrame) -> Step {
        match frame {
            ServerFrame::Chunk { request_id, values } if request_id == self.request => {
                let before = self.thread.sequence;
                for value in values {
                    self.thread.apply(value);
                }
                if self.thread.sequence == before {
                    Step::Same
                } else {
                    Step::Changed(Box::new(self.transcript()))
                }
            }
            ServerFrame::Exit { request_id, exit } if request_id == self.request => Step::Ended(
                exit.cause()
                    .unwrap_or_else(|| "T3 Code ended the thread's stream.".to_owned()),
            ),
            ServerFrame::Defect { defect } => Step::Ended(defect.to_string()),
            _ => Step::Same,
        }
    }

    /// The thread as a read-only chat transcript.
    pub(crate) fn transcript(&self) -> Transcript {
        let agent = match &self.thread.header.provider_instance_id {
            Some(provider) => format!("{provider} in T3 Code"),
            None => "T3 Code".to_owned(),
        };
        let mut transcript = sheprd_t3::transcript::transcript(&self.thread, &agent);
        // Prompts, answers, and settings stay in T3 Code for now.
        transcript.read_only = true;
        transcript
    }
}

pub(crate) fn run(options: T3Options) -> anyhow::Result<()> {
    let origin = match &options.server {
        Some(server) => url::Url::parse(server)?,
        None => {
            let home = discover::t3_home(
                std::env::var_os("T3CODE_HOME").as_deref(),
                std::env::home_dir().as_deref(),
            )
            .context("no home directory to find T3 Code in")?;
            discover::local_origin(&home)?
        }
    };
    let server = Server::new(origin);
    let descriptor = server.describe()?;
    // Only the server that issued the token may receive it.
    let token = sheprd_t3::token::load(&options.token_file, &descriptor.environment_id)?;
    let ticket = server.ticket(&token)?;
    let mut connection = Connection::open(&server.socket(&ticket)?)?;
    let request = connection.request(
        "orchestration.subscribeThread",
        json!({ "threadId": options.thread }),
    )?;
    let mut follower = Follower::new(request);
    while !follower.ready() {
        let frame = connection.next_frame()?;
        if let Step::Ended(reason) = follower.frame(frame) {
            anyhow::bail!("T3 Code refused the thread: {reason}");
        }
    }
    let mut shown = follower.transcript();
    let mut mirror = Mirror::new(shown.clone(), options.pane);

    let id = options.endpoint.unwrap_or_else(EndpointId::random);
    let address = sheprd_chat_protocol::endpoint_address(&id)
        .context("neither XDG_STATE_HOME nor HOME is set")?;
    let (inputs, receiver) = bounded(128);
    let _server = crate::attach::Server::listen(&address, inputs)?;
    // Only connection metadata goes to stdout, never thread content.
    println!("{id}");
    std::io::stdout().flush()?;

    // One transcript in flight: the worker waits while the loop takes the
    // last, which holds the server's next chunk back too.
    let (updates, mut changes) = bounded::<Step>(1);
    thread::Builder::new()
        .name("t3-thread".to_owned())
        .spawn(move || follow(connection, follower, &updates))?;
    loop {
        select! {
            recv(receiver) -> input => match input {
                Ok(Input::Shutdown) | Err(_) => break,
                Ok(input) => mirror.input(input),
            },
            recv(changes) -> step => match step {
                Ok(Step::Changed(transcript)) => {
                    shown = *transcript;
                    mirror.update(shown.clone());
                }
                Ok(Step::Ended(reason)) => {
                    shown.status = Status::Failed {
                        message: format!("The T3 Code connection ended: {reason}"),
                    };
                    mirror.update(shown.clone());
                }
                Ok(Step::Same) => {}
                // The worker is done; a disconnected channel is always
                // ready, so stop selecting on it.
                Err(_) => changes = crossbeam_channel::never(),
            },
        }
    }
    Ok(())
}

/// Reads the stream until it ends, passing each change on.
///
/// A quiet read sends one ping; a second quiet read with that ping still
/// unanswered means the server is gone even if its socket stays open.
fn follow<S: Read + Write>(
    mut connection: Connection<S>,
    mut follower: Follower,
    updates: &crossbeam_channel::Sender<Step>,
) {
    let mut pinged = false;
    loop {
        let step = match connection.next_frame() {
            Ok(frame) => {
                pinged = false;
                follower.frame(frame)
            }
            Err(sheprd_t3::Error::Idle) if pinged => {
                Step::Ended("T3 Code stopped answering.".to_owned())
            }
            // A quiet thread: check the server is still there.
            Err(sheprd_t3::Error::Idle) => match connection.ping() {
                Ok(()) => {
                    pinged = true;
                    Step::Same
                }
                Err(error) => Step::Ended(error.to_string()),
            },
            Err(error) => Step::Ended(error.to_string()),
        };
        let ended = matches!(step, Step::Ended(_));
        if !matches!(step, Step::Same) && updates.send(step).is_err() {
            return;
        }
        if ended {
            return;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;
