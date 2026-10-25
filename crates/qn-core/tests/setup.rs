//! Setup contract checks without an engine adapter or runtime dependency.
#![allow(clippy::unwrap_used, clippy::expect_used)]
use qn_core::setup::{EngineInstallation, EngineSetup, SetupError, SetupFuture, SetupOperation};
use std::{
    error::Error,
    sync::Mutex,
    task::{Context, Poll},
};

struct ReadySetup(Mutex<Vec<SetupOperation>>);
impl EngineSetup for ReadySetup {
    fn run(&self, operation: SetupOperation) -> SetupFuture<'_> {
        Box::pin(async move {
            self.0.lock().unwrap().push(operation);
            match operation {
                SetupOperation::Install => Ok(EngineInstallation {
                    engine: "test-engine",
                    version: "test-version",
                    path: "/managed/test-engine".into(),
                }),
                SetupOperation::Inspect => Err(SetupError::Unsupported),
            }
        })
    }
}
#[test]
fn it_dispatches_operations_through_a_sendable_borrowed_trait_future() {
    let setup = ReadySetup(Mutex::new(Vec::new()));
    let port: &dyn EngineSetup = &setup;
    for operation in [SetupOperation::Install, SetupOperation::Inspect] {
        let mut future = port.run(operation);
        let result = std::thread::scope(|scope| {
            scope
                .spawn(move || {
                    let waker = std::task::Waker::noop();
                    future.as_mut().poll(&mut Context::from_waker(waker))
                })
                .join()
                .unwrap()
        });
        match (operation, result) {
            (SetupOperation::Install, Poll::Ready(Ok(installation))) => {
                assert_eq!(installation.engine, "test-engine");
                assert_eq!(installation.version, "test-version");
                assert_eq!(installation.path.as_str(), "/managed/test-engine");
            }
            (SetupOperation::Inspect, Poll::Ready(Err(SetupError::Unsupported))) => {}
            (_, result) => panic!("unexpected contract result: {result:?}"),
        }
    }
    assert!(matches!(
        setup.0.into_inner().unwrap().as_slice(),
        [SetupOperation::Install, SetupOperation::Inspect]
    ));
}

#[test]
fn it_retains_a_redacted_adapter_cause_and_actionable_failure_categories() {
    let error = SetupError::Failed(Box::new(std::io::Error::other("redacted adapter failure")));
    let source = error.source().unwrap();
    assert!(source.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(source.to_string(), "redacted adapter failure");
    assert!(error.to_string().contains("qn install"));
    assert!(SetupError::Configuration.to_string().contains("QN_HOME"));
    assert!(SetupError::Unsupported.to_string().contains("WSL2"));
}
