//! **An unattended conversation, headless.** Issue #8: a conversation started with
//! `Delivery::Unattended` takes no lock and no key, and closes itself once each line has been typed and
//! up for its reading time; a modal one is unchanged.
//!
//! Time is stepped 16 ms per update, so reading times are frame counts.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    BubbleAssets, ConversationEnded, ConversationLock, ConversationProgress, Delivery,
    DialogInputSet, DialogSkin, DialogueScript, Presenter, Speaker, Speakers, SpeechBubblesPlugin,
    StartConversation, dwell_secs, validate_script,
};

#[derive(Component)]
struct TestCamera;

const STEP: Duration = Duration::from_millis(16);
const CPS: f32 = 40.0;
const LINES: [&str; 2] = ["The dead are at the wall.", "Hold the gate until the bell."];

const SCRIPT: &str = r#"
(
  conversations: {
    "toast": (
      start: "a",
      nodes: {
        "a": Line(speaker: "narrator", text: "The dead are at the wall.", next: Some("b")),
        "b": Line(speaker: "narrator", text: "Hold the gate until the bell.", next: None),
      },
    ),
    "ask": (
      start: "q",
      nodes: {
        "q": Choice(speaker: "narrator", prompt: "Ring it?", options: [
          (text: "Yes", next: "y"),
        ]),
        "y": Line(speaker: "narrator", text: "Rung.", next: None),
      },
    ),
  },
)
"#;

fn skin() -> DialogSkin {
    let slicer = bevy::sprite::TextureSlicer {
        border: BorderRect::all(4.0),
        ..default()
    };
    DialogSkin {
        frame: Handle::default(),
        frame_slicer: slicer.clone(),
        paper: Handle::default(),
        name_tag: Handle::default(),
        name_tag_slicer: slicer,
        caret: [Handle::default(), Handle::default()],
        body_font: TextFont::default(),
        title_font: TextFont::default(),
        text_color: Color::BLACK,
        choice_color: Color::BLACK,
        choice_hover_color: Color::WHITE,
        padding: 8.0,
        width: 320.0,
        portrait_size: 48.0,
        chars_per_second: CPS,
        bottom_margin: 16.0,
    }
}

/// What a host system ordered after the dialog saw: Space presses that reached it.
#[derive(Resource, Default)]
struct HostSaw(usize);

fn host_system(keys: Res<ButtonInput<KeyCode>>, mut saw: ResMut<HostSaw>) {
    if keys.just_pressed(KeyCode::Space) {
        saw.0 += 1;
    }
}

fn app(presenter: Presenter) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<HostSaw>()
        .add_plugins(SpeechBubblesPlugin::<TestCamera>::default())
        .add_systems(PreUpdate, host_system.after(DialogInputSet))
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    let script: DialogueScript = ron::from_str(SCRIPT).expect("script parses");
    validate_script(&script).expect("script validates");
    app.insert_resource(script)
        .insert_resource(presenter)
        .insert_resource(skin());
    let font = ab_glyph::FontArc::try_from_slice(include_bytes!("data/FiraMono-subset.ttf"))
        .expect("the test font parses");
    app.insert_resource(BubbleAssets {
        quad: Handle::default(),
        font,
    });
    app.world_mut()
        .spawn((TestCamera, Transform::default(), GlobalTransform::default()));
    let head = app
        .world_mut()
        .spawn((Transform::default(), Visibility::default()))
        .id();
    app.world_mut().resource_mut::<Speakers>().0.insert(
        "narrator".into(),
        Speaker {
            entity: Some(head),
            name: "Narrator".into(),
            portrait: None,
        },
    );
    app
}

fn start(app: &mut App, id: &str, delivery: Delivery) {
    app.world_mut().write_message(StartConversation {
        id: id.into(),
        delivery,
    });
    app.update();
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
}

fn ended(app: &mut App) -> Vec<ConversationEnded> {
    app.world_mut()
        .resource_mut::<Messages<ConversationEnded>>()
        .drain()
        .collect()
}

fn node(app: &App) -> Option<String> {
    app.world()
        .get_resource::<ConversationProgress>()
        .map(|p| p.node.clone())
}

/// #8, acceptance 1 and 5: no lock, the virtual clock runs, and "is a conversation up" is answered
/// apart from "is it modal".
#[test]
fn an_unattended_conversation_takes_no_lock_and_is_still_up() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "toast", Delivery::Unattended);
    assert!(app.world().get_resource::<ConversationLock>().is_none());
    let progress = app
        .world()
        .get_resource::<ConversationProgress>()
        .expect("a conversation is up");
    assert_eq!(progress.delivery, Delivery::Unattended);
    assert_eq!(progress.presses_to_close(), Some(0));
    let before = app.world().resource::<Time<Virtual>>().elapsed_secs();
    for _ in 0..10 {
        app.update();
    }
    assert!(app.world().resource::<Time<Virtual>>().elapsed_secs() > before);
}

/// #8, acceptance 2: a key pressed during an unattended conversation reaches the host and moves
/// nothing in the conversation.
#[test]
fn keys_pressed_during_an_unattended_conversation_reach_the_game() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "toast", Delivery::Unattended);
    for _ in 0..3 {
        tap(&mut app, KeyCode::Space);
    }
    assert_eq!(
        app.world().resource::<HostSaw>().0,
        3,
        "the dialog took a key"
    );
    assert_eq!(
        node(&app),
        Some("a".into()),
        "a key advanced the unattended line"
    );
    assert!(ended(&mut app).is_empty());
}

/// #8, acceptance 3: the box closes itself, no key, no sooner than each line's typing plus its
/// reading time, and ends once.
#[test]
fn an_unattended_box_closes_itself_after_its_lines_are_read() {
    for presenter in [Presenter::DialogBox, Presenter::Balloons] {
        let mut app = app(presenter);
        start(&mut app, "toast", Delivery::Unattended);
        let reading: f32 = LINES.iter().map(|l| dwell_secs(l)).sum();
        let typing: f32 = match presenter {
            Presenter::DialogBox => LINES.iter().map(|l| l.chars().count() as f32 / CPS).sum(),
            Presenter::Balloons => 0.0,
        };
        let budget = ((reading + typing) / STEP.as_secs_f32()).ceil() as usize + 120;
        let mut closed_at = None;
        for frame in 1..=budget {
            app.update();
            let closed = ended(&mut app);
            if !closed.is_empty() {
                assert_eq!(closed.len(), 1, "{presenter:?}: ended twice");
                assert_eq!((closed[0].presses, closed[0].nodes_seen), (0, 2));
                closed_at = Some(frame);
                break;
            }
        }
        let frame = closed_at.unwrap_or_else(|| panic!("{presenter:?}: never closed itself"));
        let secs = frame as f32 * STEP.as_secs_f32();
        assert!(
            secs >= reading + typing,
            "{presenter:?}: closed after {secs} s, before {reading} s of reading + {typing} s of typing"
        );
        assert!(app.world().get_resource::<ConversationProgress>().is_none());
        for _ in 0..30 {
            app.update();
        }
        assert!(ended(&mut app).is_empty(), "{presenter:?}: ended twice");
    }
}

/// #8, acceptance 4: a modal conversation is unchanged: the lock is held and Space is the dialog's.
#[test]
fn a_modal_conversation_still_locks_and_takes_its_presses() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "toast", Delivery::Modal);
    assert!(app.world().get_resource::<ConversationLock>().is_some());
    tap(&mut app, KeyCode::Space); // completes the typing line
    tap(&mut app, KeyCode::Space); // advances
    assert_eq!(
        app.world().resource::<HostSaw>().0,
        0,
        "the host saw a press the dialog used"
    );
    assert_eq!(node(&app), Some("b".into()));
}

/// Nobody can pick a choice in an unattended delivery, so such a conversation is refused at the start.
#[test]
fn an_unattended_conversation_with_a_choice_is_refused() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "ask", Delivery::Unattended);
    app.update();
    assert!(app.world().get_resource::<ConversationProgress>().is_none());
    assert!(app.world().get_resource::<ConversationLock>().is_none());
    // The refusal leaves the runtime free: a modal start right after runs.
    start(&mut app, "ask", Delivery::Modal);
    assert!(app.world().get_resource::<ConversationLock>().is_some());
}
