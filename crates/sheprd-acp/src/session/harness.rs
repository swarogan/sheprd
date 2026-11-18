//! Switching a chat to another built-in agent while its agent is idle.
//!
//! The session drops what the old agent told it (its session, modes,
//! settings, and sub-agents) but keeps the conversation, starts a new epoch
//! every client replaces its transcript with, and asks the program to start
//! the new agent ([`super::Flow::Launch`]). Once a prompt has reached an
//! agent, the one taking over gets the conversation with the user's next
//! prompt ([`super::thread`]). A switch waits for a running turn to end.

use sheprd_chat_protocol::{
    Agent, Change, Cursor, Harnesses, Rejection, ServerMessage, Transcript,
};

use super::{HarnessId, Session, Update};
use crate::rpc;

impl<R: FnMut(Update)> Session<R> {
    pub(super) fn set_agent(&mut self, agent: Agent) -> Result<(), Rejection> {
        let harnesses = self
            .transcript
            .harnesses
            .as_ref()
            .ok_or(Rejection::AgentFixed)?;
        // Mid-turn, the agent still holds work the user started.
        if self.turn.is_some() {
            return Err(Rejection::AgentFixed);
        }
        if !harnesses.available.contains(&agent) {
            return Err(Rejection::AgentMissing);
        }
        if harnesses.current != Some(agent) {
            self.switch(agent, harnesses.available.clone());
        }
        Ok(())
    }

    fn switch(&mut self, agent: Agent, available: Vec<Agent>) {
        let leaving = self
            .config
            .harness
            .zip(self.acp_session.as_ref().map(|id| id.0.to_string()));
        let resume = self
            .thread
            .switch(leaving, self.transcript.last_entry_id(), agent);
        self.config.load = resume;
        // A switch never fails for want of the earlier session: the agent
        // then starts a new one and gets the whole conversation.
        self.config.require_load = false;
        // Nothing the old agent opened or asked carries over; its answers
        // still on their way are told apart by the new id.
        self.harness = HarnessId(self.harness.0 + 1);
        self.accepts = crate::files::Accepts::default();
        self.acp_session = None;
        self.asked.clear();
        self.pending.clear();
        self.pictures = super::pictures::Pictures::default();
        self.ready = false;
        self.turn = None;
        self.config.agent = agent.display_name().to_owned();
        self.context.switched(&self.config.agent);
        self.config.harness = Some(agent);
        self.config.login_hint = Some(agent.login_hint().to_owned());

        let mut transcript = Transcript::new(self.config.agent.clone());
        transcript.auto_accept = self.transcript.auto_accept;
        transcript.clients = self.transcript.clients;
        // The conversation stays; what the old agent alone held does not.
        transcript.entries = std::mem::take(&mut self.transcript.entries);
        transcript.truncated = self.transcript.truncated;
        // Prompts typed while the old agent started go to the new one.
        transcript.queued = std::mem::take(&mut self.transcript.queued);
        transcript.harnesses = Some(Harnesses {
            available,
            current: Some(agent),
        });
        self.transcript = transcript;
        self.projector = self.projector.restart();

        // A new epoch: no client's cursor or logged change applies to the
        // new transcript, so every client replaces what it holds.
        let cursor = Cursor::new_epoch();
        self.epoch = cursor.epoch;
        self.seq = cursor.seq;
        self.log.clear();
        self.log_bytes = 0;
        let snapshot = ServerMessage::Snapshot {
            cursor,
            transcript: Box::new(self.transcript.clone()),
        };
        let before = self.remote_clients();
        self.subscribers
            .retain(|subscriber| subscriber.outbox.try_send(snapshot.clone()).is_ok());
        if self.remote_clients() != before {
            self.emit(Change::Clients {
                count: self.remote_clients(),
            });
        }
        if self.thread.started() {
            let notice = self.projector.notice(
                sheprd_chat_protocol::NoticeLevel::Info,
                format!("Switched to {}.", agent.display_name()),
            );
            self.emit(notice);
        }
        (self.report)(Update::Harness(agent));
        self.report_activity();
        self.launch = Some(agent);
    }

    /// The agent [`super::Flow::Launch`] asked for started: open the ACP
    /// connection to it.
    pub(crate) fn launched(&mut self, outbox: rpc::Outbox, pid: u32) {
        (self.report)(Update::Started(pid));
        self.outbox = outbox;
        self.start();
    }

    /// The agent [`super::Flow::Launch`] asked for could not start. The
    /// choice stays open, so another agent can still be picked.
    pub(crate) fn launch_failed(&mut self, error: &crate::Error) {
        let mut message = error.to_string();
        let mut source = std::error::Error::source(error);
        while let Some(cause) = source {
            message.push_str(": ");
            message.push_str(&cause.to_string());
            source = cause.source();
        }
        self.fail(&message);
    }
}
