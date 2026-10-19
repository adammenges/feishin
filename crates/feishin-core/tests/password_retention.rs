use feishin_core::{api::Api, model::*, storage};
use keyring::{
    credential::{Credential, CredentialApi, CredentialBuilderApi},
    mock::MockCredential,
};
use std::{any::Any, sync::Arc};

// This integration test has its own process and replaces the OS store before
// creating any entries. All entries share one disposable in-memory credential.
#[derive(Clone, Debug)]
struct MemoryStore(Arc<MockCredential>);
impl CredentialApi for MemoryStore {
    fn set_secret(&self, value: &[u8]) -> keyring::Result<()> {
        self.0.set_secret(value)
    }
    fn get_secret(&self) -> keyring::Result<Vec<u8>> {
        self.0.get_secret()
    }
    fn delete_credential(&self) -> keyring::Result<()> {
        self.0.delete_credential()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl CredentialBuilderApi for MemoryStore {
    fn build(&self, _: Option<&str>, _: &str, _: &str) -> keyring::Result<Box<Credential>> {
        Ok(Box::new(self.clone()))
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn saved_password_choice_filters_credentials_and_survives_old_request_clones() {
    let memory = MemoryStore(Arc::default());
    keyring::set_default_credential_builder(Box::new(memory.clone()));
    let server = Server {
        id: uuid::Uuid::new_v4(),
        name: "Fixture".into(),
        url: "https://fixture.invalid".into(),
        username: "listener".into(),
        kind: ServerType::Navidrome,
        legacy_auth: false,
        options: Default::default(),
    };
    let api = Api::new(
        server,
        Session {
            token: "session-token".into(),
            salt: "salt".into(),
            subsonic_token: "subsonic-token".into(),
            refresh_password: Some("fixture-password".into()),
            ..Default::default()
        },
    )
    .unwrap();
    api.remember_session().unwrap();
    let restored = storage::load_session(&api.server).unwrap();
    assert_eq!(restored.token, "session-token");
    assert_eq!(restored.salt, "salt");
    assert_eq!(restored.subsonic_token, "subsonic-token");
    assert!(restored.refresh_password.is_none());
    assert!(!memory.get_password().unwrap().contains("fixture-password"));
    assert!(api.session().refresh_password.is_some());

    let mut server = api.server.clone();
    server.options.save_password = true;
    let opted_in = api.reconfigure(server).unwrap();
    assert_eq!(
        storage::load_session(&opted_in.server)
            .unwrap()
            .refresh_password
            .as_deref(),
        Some("fixture-password")
    );
    // A clone created before the choice changed must follow the current policy.
    api.remember_session().unwrap();
    assert!(memory.get_password().unwrap().contains("fixture-password"));

    let mut server = opted_in.server.clone();
    server.options.save_password = false;
    memory.0.set_error(keyring::Error::Invalid(
        "fixture".into(),
        "write failed".into(),
    ));
    assert!(opted_in.reconfigure(server.clone()).is_err());
    // A failed write does not falsely acknowledge removal. Reading with the
    // disabled preference still refuses an older stored password.
    assert!(memory.get_password().unwrap().contains("fixture-password"));
    assert!(
        storage::load_session(&server)
            .unwrap()
            .refresh_password
            .is_none()
    );
    let opted_out = opted_in.reconfigure(server).unwrap();
    for old_request in [&api, &opted_in, &opted_out] {
        old_request.remember_session().unwrap();
        assert!(!memory.get_password().unwrap().contains("fixture-password"));
        assert_eq!(
            storage::load_session(&opted_out.server).unwrap().token,
            "session-token"
        );
    }
    opted_out.sign_out();
    assert!(api.remember_session().is_err());

    // The preference is Navidrome-specific. Jellyfin renewal and Subsonic's
    // explicitly selected legacy authentication keep their existing secrets.
    for kind in [ServerType::Jellyfin, ServerType::Subsonic] {
        let api = Api::new(
            Server {
                kind,
                ..opted_out.server.clone()
            },
            Session {
                refresh_password: Some("renewal-password".into()),
                legacy_password: Some("legacy-password".into()),
                ..Default::default()
            },
        )
        .unwrap();
        api.remember_session().unwrap();
        let restored = storage::load_session(&api.server).unwrap();
        assert_eq!(
            restored.refresh_password.as_deref(),
            Some("renewal-password")
        );
        assert_eq!(restored.legacy_password.as_deref(), Some("legacy-password"));
    }
}
