use super::*;
use core::prelude::v1::test;

#[test]
fn every_agent_has_its_own_mark_and_none_is_generic() {
    for agent in Agent::ALL {
        assert_ne!(mark(Some(*agent)), AgentIcon::Generic, "{agent}");
    }
    assert_eq!(mark(None), AgentIcon::Generic);
}
