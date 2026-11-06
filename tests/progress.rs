//! **How far a conversation has to go, what it costs, and how it ends, headless.** Issues #6 and #7:
//! `ConversationProgress::presses_to_close` against presses actually made, `SkipConversation`,
//! `DialogueScript::cost` / `fits` against hand counts, and `ConversationEnded` saying what was seen
//! on each way out.
//!
//! Time is stepped 16 ms per update and the typewriter runs at 10 characters a second, so a line of
//! twenty-odd characters is still typing for over a hundred frames: whether a press meets a typing
//! line is a frame count, not a race with the host's clock.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_speech_bubbles::{
    BubbleAssets, ConversationCost, ConversationEnded, ConversationLock, ConversationProgress,
    CostError, DialogSkin, DialogueNodeEntered, DialogueScript, Presenter, PressRule,
    SkipConversation, Speaker, Speakers, SpeechBubblesPlugin, StartConversation, validate_script,
};

#[derive(Component)]
struct TestCamera;

const STEP: Duration = Duration::from_millis(16);
/// Slow enough that no line below finishes typing within the frames a test spends on it.
const SLOW_CPS: f32 = 10.0;

const SCRIPT: &str = r#"
(
  conversations: {
    "linear": (
      start: "a",
      nodes: {
        "a": Line(speaker: "narrator", text: "The first of three lines.", next: Some("b")),
        "b": Line(speaker: "narrator", text: "The second of three lines.", next: Some("c")),
        "c": Line(speaker: "narrator", text: "The last of three lines.", next: None),
      },
    ),
    "branch": (
      start: "start",
      nodes: {
        "start": Line(speaker: "narrator", text: "The gate is shut tonight.", next: Some("ask")),
        "ask": Choice(speaker: "narrator", prompt: "Open the gate?", options: [
          (text: "Leave it", next: "x"),
          (text: "Open it", next: "y"),
        ]),
        "x": Line(speaker: "narrator", text: "You leave it shut for now.", next: None),
        "y": Line(speaker: "narrator", text: "The hinges creak and give.", next: Some("y2")),
        "y2": Line(speaker: "narrator", text: "Cold air comes through it.", next: None),
      },
    ),
    "menu": (
      start: "ask",
      nodes: {
        "ask": Choice(speaker: "narrator", prompt: "Again?", options: [
          (text: "Again", next: "again"),
          (text: "Done", next: "done"),
        ]),
        "again": Line(speaker: "narrator", text: "Once more.", next: Some("ask")),
        "done": Line(speaker: "narrator", text: "Done.", next: None),
      },
    ),
  },
)
"#;

fn skin(chars_per_second: f32) -> DialogSkin {
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
        chars_per_second,
        bottom_margin: 16.0,
    }
}

fn script() -> DialogueScript {
    let script: DialogueScript = ron::from_str(SCRIPT).expect("script parses");
    validate_script(&script).expect("script validates");
    script
}

fn app(presenter: Presenter) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<StandardMaterial>()
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_plugins(SpeechBubblesPlugin::<TestCamera>::default())
        .insert_resource(TimeUpdateStrategy::ManualDuration(STEP))
        .insert_resource(script())
        .insert_resource(presenter)
        .insert_resource(skin(SLOW_CPS));
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

fn start(app: &mut App, id: &str) {
    app.world_mut()
        .write_message(StartConversation { id: id.into() });
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

fn progress(app: &App) -> Option<ConversationProgress> {
    app.world().get_resource::<ConversationProgress>().cloned()
}

fn presses_to_close(app: &App) -> Option<u32> {
    progress(app).and_then(|p| p.presses_to_close())
}

fn ended(app: &mut App) -> Vec<ConversationEnded> {
    app.world_mut()
        .resource_mut::<Messages<ConversationEnded>>()
        .drain()
        .collect()
}

fn entered(app: &mut App) -> Vec<String> {
    app.world_mut()
        .resource_mut::<Messages<DialogueNodeEntered>>()
        .drain()
        .map(|e| e.node)
        .collect()
}

const TYPED_BOX: PressRule = PressRule::Boxed { typewriter: true };

/// #6, acceptance 1: three boxed lines, the first typing, cost two presses each (complete, advance);
/// once the first has finished typing, one fewer.
#[test]
fn presses_to_close_counts_the_typewriter_press() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "linear");
    let p = progress(&app).expect("progress while a conversation runs");
    assert!(p.typing, "the first line should still be typing: {p:?}");
    assert_eq!((p.node.as_str(), p.line, p.lines_left), ("a", 1, 3));
    assert_eq!(p.presses_to_close(), Some(6));

    tap(&mut app, KeyCode::Space); // completes the first line
    let p = progress(&app).expect("still running");
    assert!(!p.typing);
    assert_eq!(p.presses_to_close(), Some(5));
}

/// #6, acceptance 2: exactly `presses_to_close` presses close it; one fewer does not.
#[test]
fn pressing_exactly_presses_to_close_times_closes_it() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "linear");
    let n = presses_to_close(&app).expect("a linear conversation has a count");
    for i in 1..n {
        tap(&mut app, KeyCode::Space);
        assert!(
            ended(&mut app).is_empty(),
            "closed after {i} of {n} presses"
        );
        assert_eq!(
            presses_to_close(&app),
            Some(n - i),
            "the count did not go down by one per press"
        );
    }
    tap(&mut app, KeyCode::Space);
    let closed = ended(&mut app);
    assert_eq!(closed.len(), 1, "the last press should close it once");
    assert_eq!(closed[0].presses as u32, n);
    assert!(
        progress(&app).is_none(),
        "progress outlived the conversation"
    );
}

/// #6, edge case: a timed balloon line takes no press.
#[test]
fn timed_balloon_lines_cost_no_press() {
    let mut app = app(Presenter::Balloons);
    start(&mut app, "linear");
    assert_eq!(presses_to_close(&app), Some(0));
    assert_eq!(progress(&app).map(|p| p.lines_left), Some(3));
}

/// #6, acceptance 3: a skip closes a linear conversation once, announces every node it passes, says it
/// was skipped, and leaves the speakers registered.
#[test]
fn a_skip_closes_a_linear_conversation_once() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "linear");
    assert_eq!(entered(&mut app), vec!["a"]);
    app.world_mut().write_message(SkipConversation);
    app.update();
    assert_eq!(entered(&mut app), vec!["b", "c"]);
    let closed = ended(&mut app);
    assert_eq!(closed.len(), 1, "{closed:?}");
    assert_eq!(
        (
            closed[0].id.as_str(),
            closed[0].nodes_seen,
            closed[0].presses,
            closed[0].skipped
        ),
        ("linear", 3, 0, true)
    );
    assert!(app.world().get_resource::<ConversationLock>().is_none());
    assert!(app.world().resource::<Speakers>().get("narrator").is_some());
    for _ in 0..3 {
        app.update();
    }
    assert!(ended(&mut app).is_empty(), "the skip ended it twice");
}

/// #6, acceptance 4: a choice ahead makes the count `None`, while `lines_left` counts the lines to it;
/// after the pick the count is back.
#[test]
fn a_choice_ahead_has_no_count_until_it_is_picked() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "branch");
    let p = progress(&app).expect("running");
    assert_eq!((p.presses_to_close(), p.lines_left), (None, 1));
    tap(&mut app, KeyCode::Space); // completes
    tap(&mut app, KeyCode::Space); // advances onto the choice
    assert_eq!(progress(&app).map(|p| p.node), Some("ask".into()));
    assert_eq!(presses_to_close(&app), None);
    tap(&mut app, KeyCode::Digit2);
    let p = progress(&app).expect("running");
    assert_eq!(p.node, "y");
    assert!(p.typing);
    assert_eq!(
        (p.presses_to_close(), p.lines_left, p.line),
        (Some(4), 2, 2)
    );
}

/// A skip stops at a choice: it presents it and leaves the pick to the player.
#[test]
fn a_skip_stops_at_the_next_choice() {
    let mut app = app(Presenter::DialogBox);
    start(&mut app, "branch");
    entered(&mut app);
    app.world_mut().write_message(SkipConversation);
    app.update();
    assert_eq!(entered(&mut app), vec!["ask"]);
    assert!(ended(&mut app).is_empty());
    assert_eq!(progress(&app).map(|p| p.node), Some("ask".into()));
    tap(&mut app, KeyCode::Digit1);
    assert_eq!(progress(&app).map(|p| p.node), Some("x".into()));
}

/// #7, acceptance 1 and 2: the cost from the script alone equals a hand count, under each rule.
#[test]
fn cost_from_the_script_matches_a_hand_count() {
    let s = script();
    let instant = PressRule::Boxed { typewriter: false };
    // Three lines: two presses each typed, one each shown at once, none timed.
    for (rule, presses) in [(TYPED_BOX, 6), (instant, 3), (PressRule::Timed, 0)] {
        assert_eq!(
            s.cost("linear", rule),
            Ok(ConversationCost {
                lines: 3,
                presses,
                branches: false
            }),
            "{rule:?}"
        );
    }
    // start (2) + the choice (2: complete the prompt, pick) + the longer branch y, y2 (2 + 2).
    assert_eq!(
        s.cost("branch", TYPED_BOX),
        Ok(ConversationCost {
            lines: 3,
            presses: 8,
            branches: true
        })
    );
    // Timed: only the pick is a press.
    assert_eq!(s.cost("branch", PressRule::Timed).map(|c| c.presses), Ok(1));
    assert!(s.fits("linear", 6, TYPED_BOX));
    assert!(!s.fits("linear", 5, TYPED_BOX));
    assert!(s.fits("linear", 3, instant));
}

/// #7, acceptance 3 and the loop edge case: an unknown id and a loop are errors that name them.
#[test]
fn cost_refuses_an_unknown_or_looping_conversation_by_name() {
    let s = script();
    let unknown = s.cost("nope", TYPED_BOX);
    assert_eq!(unknown, Err(CostError::UnknownConversation("nope".into())));
    assert!(unknown.is_err_and(|e| e.to_string().contains("'nope'")));
    assert!(!s.fits("nope", usize::MAX, TYPED_BOX));
    assert!(matches!(
        s.cost("menu", TYPED_BOX),
        Err(CostError::Unbounded { ref conversation, .. }) if conversation == "menu"
    ));
    assert!(!s.fits("menu", usize::MAX, TYPED_BOX));
}

/// The runtime charges what `cost` predicted: a linear conversation read press by press ends with
/// `presses` equal to its cost and every node seen.
#[test]
fn the_runtime_charges_what_cost_predicted() {
    let mut app = app(Presenter::DialogBox);
    let predicted = script()
        .cost("linear", TYPED_BOX)
        .expect("linear has a cost");
    start(&mut app, "linear");
    for _ in 0..predicted.presses {
        tap(&mut app, KeyCode::Space);
    }
    let closed = ended(&mut app);
    assert_eq!(closed.len(), 1);
    assert_eq!(
        (closed[0].presses, closed[0].nodes_seen, closed[0].skipped),
        (predicted.presses, 3, false)
    );
}

/// #7, acceptance 5: a node entered twice is seen once.
#[test]
fn nodes_seen_counts_a_re_entered_node_once() {
    let mut app = app(Presenter::DialogBox);
    app.insert_resource(skin(0.0));
    start(&mut app, "menu");
    app.update(); // the prompt is shown whole
    tap(&mut app, KeyCode::Digit1); // again
    tap(&mut app, KeyCode::Space); // back to ask
    tap(&mut app, KeyCode::Digit2); // done
    tap(&mut app, KeyCode::Space); // end
    let closed = ended(&mut app);
    assert_eq!(closed.len(), 1, "{closed:?}");
    // ask, again, done: `ask` was entered twice.
    assert_eq!((closed[0].nodes_seen, closed[0].presses), (3, 4));
}

/// #7, acceptance 4 (the crate's own exits): a missing dialog skin ends it once, with what was seen.
#[test]
fn a_refused_conversation_ends_once_with_what_was_seen() {
    let mut app = app(Presenter::DialogBox);
    app.world_mut().remove_resource::<DialogSkin>();
    start(&mut app, "linear");
    app.update();
    let closed = ended(&mut app);
    assert_eq!(closed.len(), 1, "{closed:?}");
    assert_eq!(
        (closed[0].nodes_seen, closed[0].presses, closed[0].skipped),
        (1, 0, false)
    );
    assert!(app.world().get_resource::<ConversationLock>().is_none());
    assert!(progress(&app).is_none());
}
