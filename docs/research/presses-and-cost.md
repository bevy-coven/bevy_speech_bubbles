# Presses, skips and a conversation's cost

Issues #6 (how many presses close a conversation, and a skip) and #7 (what a conversation costs before it
starts, and what was read after). Hand-driven pass, owner request 2026-11-06, host bmb.

## Sources

- **Model / shipping game:** Yarn Spinner, *Line Advancer* (docs.yarnspinner.dev/components/dialogue-view/dialogue-advance-input),
  read 2026-11-06: the three requests a player's press makes of a line.
- **Technical:** *Longest path problem*, §Acyclic graphs (en.wikipedia.org/wiki/Longest_path_problem), read
  2026-11-06: longest paths are linear-time on a DAG and NP-hard on a general graph.
- From the corpus, already quoted in `barks.md`: Gregory, *Game Engine Architecture* 3e, §14.6.2.3, a
  "simple, fire-and-forget interface" for dialog.

## Craft principle

A press on a line is not one thing. Yarn Spinner's Line Advancer:

> "A **Line Advancer** listens for user input and sends requests to a Dialogue Runner to advance the
> presentation of the current line, either by asking a dialogue runner to hurry up its delivery, advance to
> the next line, or cancel the entire dialogue session."

So the unit a host must count is the request, not the line: a typing line takes a "hurry up" and then a
"next line", and "cancel the entire dialogue session" is a third request distinct from both.

## How shipping games do it

Yarn Spinner exposes all three as separate inputs ("Hurry Up Line", "Next Line", "Cancel Dialogue") and
calls the pattern "a 'press spacebar to continue/skip' mechanic". This crate's dialog box already does the
first two on one key (`complete_typing`, then `advance_line`); it lacked a count of them and the third.

## Model adopted

- **`PressRule`** names the host's settings once: `Timed` (balloons: a line takes no press, a choice its
  pick) or `Boxed { typewriter }` (every line one press, one more first while it types). Built with
  `PressRule::new(presenter, chars_per_second)` so a cost asked before a start is the one the runtime
  charges.
- **The count is an upper bound**: a player who confirms as soon as a node is up. A press that lands after
  a line has finished typing on its own saves one; a number key picks a choice whose prompt is typing. A
  host gates on a guarantee.
- **`ConversationProgress`** (a resource that exists exactly while a conversation runs) carries the cursor,
  `line`, `lines_left`, `typing` and `presses_to_close()`, `None` while a choice lies ahead.
- **`SkipConversation`** is the "cancel" request, but it runs the end-of-line path: every node it passes
  is entered through the one `Active::enter` (so `DialogueNodeEntered` fires as presses would), it stops at a
  choice, and it ends through the one `finish`.
- **`DialogueScript::cost`** is a longest path over the node graph by post-order, each node once, with an
  explicit stack; `lines` and `presses` are each a maximum over paths. A back edge is `CostError::Unbounded`:
  on a graph with a cycle the walk length has no bound, and the simple-path version is NP-hard ("In
  contrast to the shortest path problem ... the longest path problem is NP-hard"), so refusing is the
  honest answer. "If G is a directed acyclic graph (DAG) ... a longest path in G can be found in linear time."
- **`ConversationEnded`** carries `nodes_seen` (distinct), `presses` and `skipped`. One message for one event:
  issue #7's proposed second message, `DialogueFinished`, would be written at the same place as
  `ConversationEnded`, for the same event.

## Parameters

| Parameter | Value | Source |
|---|---|---|
| presses per typed boxed line | 2 | the dialog box's `complete_typing` then `advance_line` |
| presses per boxed line shown at once (`chars_per_second <= 0`) | 1 | `typewriter_tick` reveals it whole |
| presses per timed balloon line | 0 | `advance_line` advances on `advance_at` |
| presses per choice | 1 pick, + 1 while a boxed prompt types | `choice_focus_input` |

## Validation targets

1. Three typed boxed lines report `Some(6)` while the first types, `Some(5)` once it is typed.
2. Exactly `presses_to_close` presses close it, the count falling by one per press; one fewer does not.
3. A skip closes a linear conversation once (`skipped: true`, all nodes seen), and stops at a choice.
4. A choice ahead reports `None`; after the pick the count returns.
5. `cost` matches hand counts under each rule; an unknown id and a loop are errors naming them.
6. The runtime's `ConversationEnded::presses` equals `cost` for a conversation read press by press.
7. A node entered twice is seen once.

## What great looks like

An agent or a replay reads one number and presses that many times, or writes one message, and the card is
gone with every node effect fired; a host asks `fits(id, 3, rule)` before opening a villager's line on a
busy night and gets a guarantee, not a guess.

## Deviations

- **#6 acceptance 1 asks `Some(L + 1)` while typing and `Some(L)` after.** That counts only the line on
  screen as typing; every later boxed line types too, so a player pressing at once needs `2L`. The crate
  reports `2L` and `2L - 1` (a box with no typewriter: `L`), which is what makes acceptance 2 hold for an
  agent pressing once per frame. Assumed decision, recorded on #6.
- **#7's `DialogueFinished` is `ConversationEnded`'s new fields** (`nodes_seen`, `presses`, `skipped`), not a
  second message (one path). Breaking: `ConversationEnded` gains fields. Assumed decision, recorded on #7.
- **#7's exits "Escape" and "a board opening over it"** are the host's; this crate's ways out (last line, a
  skip, a missing skin or assets, a missing node) all pass through `finish`, and a host's own Escape writes
  `SkipConversation`.
- **`cost` takes a `PressRule`.** #7 counts "a boxed line two presses, a plain line one" per line; here the
  presenter is one rule for the whole game, so the rule is an argument.

## Pending index

- home-still was unreachable this pass (`hs distill search`: "No route to host" for 192.168.1.110:7434; the
  MCP bridge not connected), so no corpus source on reading-press cost or branching-dialogue structure was
  searched. Next pass: `distill_search "dialogue skip typewriter press to continue"` and
  `"branching dialogue graph structure path length"`, and index the Yarn Spinner docs.
