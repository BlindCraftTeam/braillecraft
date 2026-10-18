# BrailleCraft script for Orca

The `braillecraft-desktop` folder is an Orca application script: it gives Orca, in BrailleCraft on Linux, what the
BrailleCraft JAWS scripts give JAWS. Orca names an application script after the application's accessibility name,
which for BrailleCraft is `braillecraft-desktop` however it was started.

## Installing

Orca 45 to 50 look for user scripts in the `orca-scripts` package of Orca's preferences folder. Run, as the user who
runs Orca:

```
sh /opt/braillecraft/orca/install.sh
```

(from the .deb or .rpm; from the AppImage or the plain folder, run `install.sh` from the `orca` folder beside the
program). It copies the script to `~/.local/share/orca/orca-scripts/braillecraft-desktop`. Then restart Orca.

Orca 51 and later no longer look there; they load user extensions from `~/.local/share/orca/extensions` after the
user approves them. A BrailleCraft extension for those versions is a separate item of the plan.

## The keys, each with Alt, in a BrailleCraft document window

| Key | What it does |
|---|---|
| 7 | The translated line |
| 8 | The current style; twice quickly in a table, the row and column |
| 9 | The location: page, line and column |
| 0 | The document name and the status line |
| Comma | The position on the line; twice quickly, the characters to the end of the line |
| Period | The number of characters on the line; twice quickly, the left spaces |
| End | The last word on the line |
| Semicolon | The translated line read as the review place, and back |
| U | The speaking level for up and down in a braille document: silent, braille characters, the translated line |
| I | The style change and skipped line announcements |
| P | The echo for entry in a braille document |
| W | The reading of the completed braille word when you press space |
| C | Whether a braille keyboard is connected |
| Up, Down, Left, Right | The table entry moves, with the entry spoken |
| T | The current table entry |
| H | This list |

In a braille document the characters, words and lines the caret moves over are spoken as braille cell names, and
contracted braille is off while the editing area has focus, so a braille display shows the document's cells; your
own choice comes back when you leave. The choices the keys change are kept for the Orca session.
