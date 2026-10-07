//! Headless reactive-lifecycle regression, using the real executor and Suspend.
//! DOM geometry and native browser history remain separate browser checks.
use super::*;
use leptos::{
    reactive::effect::RenderEffect,
    tachys::{
        reactive_graph::Suspend,
        renderer::types::{Element, Node},
        view::{Mountable, Render},
    },
    task::Executor,
};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

#[derive(Debug, PartialEq, Eq)]
struct EntryReads {
    submit: [bool; 2],
    status: &'static str,
    admission: bool,
    code: String,
    readonly: bool,
    account: bool,
}
impl EntryReads {
    // These are the Controller reads consumed by OnlinePanel's text,
    // properties, conditional content and admission controls.
    fn read(controller: Controller) -> Self {
        Self {
            submit: [Action::Create, Action::Join].map(|action| controller.can_submit(action)),
            status: controller.status_key(),
            admission: controller.visible_admission().is_some(),
            code: controller
                .snapshot()
                .map(|state| state.code)
                .unwrap_or_default(),
            readonly: controller.snapshot().is_none_or(|state| state.busy()),
            account: controller.account_snapshot().is_some(),
        }
    }
    fn retired() -> Self {
        Self {
            submit: [false; 2],
            status: "online.context_changed",
            admission: false,
            code: String::new(),
            readonly: true,
            account: false,
        }
    }
}

struct EntryView {
    controller: Option<Controller>,
    reads: Arc<AtomicUsize>,
    replaced: Arc<AtomicBool>,
}
struct EntryViewState {
    reader: Option<RenderEffect<EntryReads>>,
}
impl Render for EntryView {
    type State = EntryViewState;

    fn build(self) -> Self::State {
        EntryViewState {
            reader: self.controller.map(|controller| {
                RenderEffect::new(move |_| {
                    self.reads.fetch_add(1, Ordering::SeqCst);
                    EntryReads::read(controller)
                })
            }),
        }
    }
    fn rebuild(self, state: &mut Self::State) {
        let reader = state.reader.as_ref().expect("old entry is still retained");
        assert_eq!(
            reader.with_value_mut(|value| value == &EntryReads::retired()),
            Some(true),
            "queued old entry reads ran safely before replacement"
        );
        self.replaced.store(true, Ordering::SeqCst);
        *state = self.build();
    }
}
// This only carries retained render effects. No DOM operations are requested;
// the actual Tachys Suspend rebuild supplies the ownership/tick boundary.
impl Mountable for EntryViewState {
    fn unmount(&mut self) {
        panic!("lifecycle test unexpectedly requested DOM unmount");
    }
    fn mount(&mut self, _: &Element, _: Option<&Node>) {
        panic!("lifecycle test unexpectedly requested DOM mount");
    }
    fn insert_before_this(&self, _: &mut dyn Mountable) -> bool {
        panic!("lifecycle test unexpectedly requested DOM insertion");
    }
}

#[test]
fn retained_entry_reads_survive_router_suspend_replacement_tick() {
    Executor::init_futures_executor().expect("native lifecycle test uses the existing executor");
    let app = Owner::new();
    app.with(|| {
        provide_context(crate::views::LocaleSignal::new(tabula_registry::Locale::En));
        crate::account::provide_account_session();
        provide_online_session();
        #[cfg(feature = "account-social")]
        crate::social_full::provide_social();
    });
    let route = app.child();
    let controller = route.with(|| use_controller("test.module".into()));
    controller.edit_code("abcd2345efgh");
    let reads = Arc::new(AtomicUsize::new(0));
    let replaced = Arc::new(AtomicBool::new(false));
    let initial = EntryView {
        controller: Some(controller),
        reads: Arc::clone(&reads),
        replaced: Arc::clone(&replaced),
    };
    let mut retained = route.with(|| Suspend::new(async move { initial }).build());
    Executor::poll_local();
    assert!(reads.load(Ordering::SeqCst) > 0);
    assert_eq!(controller.snapshot().unwrap().code, "ABCD2345EFGH");
    let replacement = EntryView {
        controller: None,
        reads: Arc::clone(&reads),
        replaced: Arc::clone(&replaced),
    };
    let old_owner = route.clone();
    app.with(|| {
        Suspend::new(async move {
            // NestedRoutesView resolves the replacement, cleans the old owner,
            // then Suspend::rebuild waits a real executor tick before remounting.
            old_owner.cleanup();
            replacement
        })
        .rebuild(&mut retained);
    });
    for _ in 0..100 {
        Executor::poll_local();
        if replaced.load(Ordering::SeqCst) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(replaced.load(Ordering::SeqCst), "replacement completed");
    assert!(reads.load(Ordering::SeqCst) >= 2, "retired callback ran");
    assert_eq!(EntryReads::read(controller), EntryReads::retired());
    // Events already queued against the old controls are inert after retirement.
    controller.edit_code("SHOULDNOTSAVE");
    controller.submit(Action::Create);
    controller.submit(Action::Join);
    controller.open(tabula_registry::Locale::En);
    controller.account.recheck();
    let next_route = app.child();
    let next = next_route.with(|| use_controller("test.module".into()));
    assert_eq!(next.snapshot().unwrap().code, "ABCD2345EFGH");
    assert!(!next.can_submit(Action::Create));
    assert!(!next.can_submit(Action::Join));
    assert!(next.visible_admission().is_none());
    drop(retained);
    next_route.cleanup();
    Executor::poll_local();
}
