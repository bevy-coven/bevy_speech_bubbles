# Delivery: a conversation the world does not wait for

Issue #8. Hand-driven pass, owner request 2026-11-06, host bmb.

## Sources

- **Model / shipping game:** Yarn Spinner, *Line Presenter* (yarnspinner.dev/docs/unity/04-showing-dialogue/02-line-presenter/),
  read 2026-11-06: the Auto Advance setting.
- **Shipping game (corpus):** Gregory, *Game Engine Architecture* 3e, §14.6.2.3, quoted in `barks.md`: dialog
  wants "a simple, fire-and-forget interface".
- **Technical:** this crate's input routing (`DialogInputSet`, `clear_just_pressed` only for a press the
  runtime used; `docs/guide/timing.md`).

## Craft principle

A line the player is not asked to act on still has to be read, so the time it stays up is its reading
time, counted from when it is all on screen. Yarn Spinner's Line Presenter:

> "To move on without the player pressing anything, turn on **Auto Advance**"

> "Auto Advance | Off | Moves on by itself once a line has finished typing."

## How shipping games do it

Yarn Spinner makes auto-advance a property of the presenter, with the same typewriter and the same panel as
a line that waits for a press: one renderer, two pacings. Gregory's "fire-and-forget" is the host side: the
caller says what to show and is not asked to drive it.

## Model adopted

- **`Delivery`, per conversation, on `StartConversation`**: `Modal` (the default; today's behaviour,
  `ConversationLock` held, presses read) or `Unattended`.
- **Unattended**: no `ConversationLock`; `advance_line` reads and consumes no key or click; a line advances
  once its typewriter has finished and it has been up for `dwell_secs(text)` (the crate's reading-speed rule,
  Brysbaert 2019), in the box or in balloons. The same presenter draws it.
- **"Is a conversation up"** is `ConversationProgress` (present while any runs); **"is it modal"** is
  `ConversationLock`.
- **Refused at the start**: an unattended conversation with a choice (nobody can pick) or a loop (it would
  never close), found with `DialogueScript::cost`'s shape; a `warn!` names it.
- **Two at once**: the runtime's existing rule, one conversation at a time, a start while one runs is
  ignored; a host that wants hints queued keeps its queue (Sanctuary's does today).

## Parameters

| Parameter | Value | Source |
|---|---|---|
| unattended line time | typing + `dwell_secs(text)` | Yarn's Auto Advance; `dwell_secs` (Brysbaert 2019) |
| default delivery | `Modal` | #8: "unchanged and the default" |

## Validation targets

1. Unattended: no `ConversationLock`; `Time<Virtual>` advances; `ConversationProgress` present, `delivery`
   `Unattended`, `presses_to_close() == Some(0)`.
2. Space during an unattended conversation reaches a host system ordered after `DialogInputSet`, and the
   line does not move.
3. It closes itself, once, no sooner than typing plus reading time, in the box and in balloons.
4. Modal: the lock is held and presses are the dialog's.
5. An unattended conversation with a choice is refused and leaves the runtime free.

## What great looks like

A host's busy-moment toast and its calm-moment card are one `StartConversation` with two deliveries: same
box, same typewriter, same font; the night's first wave marches while the card reads itself out and goes.

## Deviations

- **`Delivery::Until { seconds }`** (optional in #8) is not built: one pacing rule (typing + reading time) is
  the whole of `Unattended`; a deadline is a second rule a host can add by writing `SkipConversation` when
  its own timer runs out.
- **A modal start while an unattended conversation runs is ignored**, as any start is while one runs. A
  replace-or-queue rule is a follow-up (recorded on #8).

## Pending index

- home-still was unreachable this pass (no route to 192.168.1.110); no corpus search for non-modal dialogue or
  subtitle timing was run. Next pass: `distill_search "subtitle display duration reading time games"`.
