# Meiki user guide

Meiki is a personal, network-free desktop application for typed-cloze recall.
It does not require an account or network connection.

## Create a card

Open **Add / Edit**, type or paste source text, select the answer span, and
choose **Make cloze**. Add accepted alternatives, a hint, annotations,
explanation, audio, or an image if useful. Preview each cloze, then choose
**Save**. One source note can contain several independently scheduled clozes.

Text is stored as entered. Answer comparison normalizes Unicode to NFC and
trims outer whitespace by default; case, accents, punctuation, width, and
internal spacing remain significant unless you select a more forgiving
matching policy.

## Study

Open **Today** and choose **Start study**. Today starts with **All decks** and
can optionally filter to one deck. Type the missing text and press Enter.
After the answer is revealed, Enter accepts the suggested grade.
Scheduled reviews appear only when their exact due time has arrived. The local
day boundary controls daily limits; it does not make a later review available
early. Today shows the next scheduled time when no review is currently due.

| Key           | Action                                                |
| ------------- | ----------------------------------------------------- |
| Enter         | Check the answer; after reveal, accept the suggestion |
| 1 / 2 / 3 / 4 | Again / Hard / Good / Easy                            |
| R             | Replay audio                                          |
| E             | Edit the current note                                 |
| S             | Suspend the card                                      |
| Cmd/Ctrl+Z    | Undo the last review                                  |

Enter does nothing while an IME composition is active. A near match provides
feedback but is not silently accepted.

## Set a daily study budget

Open **Settings** and choose the collection's daily study time. A deck inherits
that budget unless you enable its deck override. Automatic scheduling previews
the derived retention target and new-card intake before you save.

Meiki always shows every review whose exact due time has arrived. If due work
already exceeds the budget, Settings and Today report the backlog and
automatic mode pauses new cards before reducing retention. The day boundary
defines the local study day and remains correct across short and long
daylight-saving transitions.

Choose **Expert** only when you want manual target retention, new-card maximum,
maximum interval, or versioned memory-parameter import/export. Memory
parameters and the time-budget policy are separate; policy changes affect only
future scheduling decisions and never rewrite history.

## Import and export language bundles

Open **Decks** and choose **Import bundle** to preview a `.meiki` language
bundle. The preview lists its ordered decks and marks stages that are already
installed. Adding a bundle preserves existing decks and study state; imported
cards start unseen with Automatic scheduling.

Use **Bundle actions** to export the remaining installed decks for a language
or to remove that language with one confirmation. A clean export includes the
bundle's active cards and local media, but not review history, current due
dates, the collection study-time setting, unrelated decks, or Trash.

## Recovery and media

If an action is interrupted, Meiki keeps the complete pending review command
and presents **Try again**. A restart retries the same command identity, so a
saved review is not duplicated when its response was lost. Before displaying
the next cached card, Meiki checks the current schedule and skips
cards that were changed, suspended, moved, deleted, or rescheduled. Missing or
corrupt media is reported without blocking study.

Keep exported `.meiki` bundles wherever you want to install them again. Meiki
also creates internal recovery points for migrations and transactional bundle
operations; these are maintained automatically and are not shown in Settings.
