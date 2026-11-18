use super::*;

fn on(route: Route) -> Target {
    Target {
        existing: None,
        endpoint: EndpointId::random(),
        pane_id: "p_1".into(),
        route,
        surface: Surface::Herdr,
    }
}

#[test]
fn a_chat_on_another_machine_says_it_is_typed_from_elsewhere() {
    use sheprd_chat_protocol::{Device, Via};
    assert_eq!(on(Route::Local).origin(), TYPED_HERE);
    let ssh = on(Route::Ssh("devbox".into())).origin();
    assert_eq!((ssh.device, ssh.via), (Device::Desktop, Via::Remote));
    assert!(!ssh.opens_paths());
}
