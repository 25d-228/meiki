# Meiki user guide

Meiki is a local-first desktop application for typed-cloze recall. It does not
require an account or network connection.

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

Open **Today** and choose **Start study**. Type the missing text and press
Enter. After the answer is revealed, Enter accepts the suggested grade.
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

## Back up and restore

Open **Settings → Archives and recovery** to export a complete collection or a
deck. Library can export selected notes. A `.meiki` archive preserves review
history, schedule metadata, Unicode text, and checksum-addressed media.

Always preview an import. **Merge** keeps the current collection and
deterministically remaps colliding identities. **Replace** is available only
for a complete collection and requires typing `REPLACE`. Meiki creates a
recovery backup before either mode changes the collection.

Settings lists rolling backups. Restoring one requires its exact filename and
creates another recovery point for the collection being replaced.

## Recovery and media

If an action is interrupted, Meiki keeps the complete pending review command
and presents **Try again**. A restart retries the same command identity, so a
saved review is not duplicated when its response was lost. Before displaying
the next cached card, Meiki checks the current database schedule and skips
cards that were changed, suspended, moved, deleted, or rescheduled. Missing or
corrupt media is reported without blocking study.

Keep exported `.meiki` archives on another device or backup volume. The
runtime SQLite database is not the portable format.
