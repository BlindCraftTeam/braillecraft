"""Writes tables/languages.tsv, BrailleMaker's list of translation table sets by language: the list shown under
Document, Translation Tables, and the learning steps shown under Document, Learning Tables.

Usage: python3 -I make_language_list.py <language list> <language names> <out.tsv>

<language list> is the decoded language list text (one section per language: the language, its regions, a help page,
then "code/RULES+SCAN/RULES+SCAN", then learning steps "TABLE: shown name"). <language names> maps the language
words of that list to their English names (tab-separated: word, string number, name). Both are read from outside the
repository.

How a line is read: anything from "| " on is a comment; blank lines are skipped; a line with the four table names
("code/A+B/C+D") gives the tables; lines starting "eBrailleLanguage: " or "eBrailleSystem: " are skipped; a line with
".htm" is the help page (not kept); a line with ":" is a learning step (table before the colon, shown name after it);
the line after the language is its regions; any other line starts a new language.
"""
import sys

REGIONS = ["America", "Europe", "Asia", "Africa", "Pacific", "Debug"]


def main(list_path, names_path, out_path):
    names = {}
    for line in open(names_path, encoding="utf-8"):
        f = line.rstrip("\n").split("\t")
        if len(f) >= 3:
            names[f[0]] = f[2]
    entries, cur, expect_regions = [], None, False
    for raw in open(list_path, encoding="latin-1"):
        line = raw.rstrip("\r\n")
        cut = line.find("| ")
        if cut >= 0:
            line = line[:cut]
        line = line.strip()
        if not line:
            continue
        parts = line.split("/")
        if len(parts) == 3 and parts[1].count("+") == 1 and parts[2].count("+") == 1:
            p2b, p2b_scan = parts[1].split("+")
            b2p, b2p_scan = parts[2].split("+")
            cur["code"] = parts[0].strip()
            cur["tables"] = [t.strip().lower() for t in (p2b, p2b_scan, b2p, b2p_scan)]
            continue
        if line.startswith("eBrailleLanguage: ") or line.startswith("eBrailleSystem: "):
            continue
        if ".htm" in line:
            continue
        if ":" in line:
            table, _, shown = line.partition(":")
            cur["steps"].append((table.strip().lower(), shown.strip()))
            continue
        if expect_regions:
            cur["regions"] = [r for r in REGIONS if r.lower() in line.lower()]
            expect_regions = False
            continue
        word = line.replace('"', "").replace("\\", "")
        cur = {"name": names.get(word, word), "regions": [], "code": "", "tables": [], "steps": []}
        entries.append(cur)
        expect_regions = True
    with open(out_path, "w", encoding="utf-8", newline="\n") as out:
        out.write("# Translation table sets by language, in list order (the number of a line, from 0, is its entry).\n")
        out.write("# Columns, tab-separated: shown name; regions; language code; print-to-braille rules and scan table;\n")
        out.write("# braille-to-print rules and scan table; learning steps as TABLE=shown name, separated by \" | \".\n")
        for e in entries:
            if len(e["tables"]) != 4:
                sys.exit("no tables for " + e["name"])
            steps = " | ".join(f"{t}={s}" for t, s in e["steps"])
            out.write("\t".join([e["name"], ",".join(e["regions"]), e["code"], *e["tables"], steps]) + "\n")
    print(f"{len(entries)} languages, {sum(len(e['steps']) for e in entries)} learning steps")


if __name__ == "__main__":
    main(*sys.argv[1:4])
