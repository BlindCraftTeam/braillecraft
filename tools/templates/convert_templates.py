"""Writes BrailleCraft's document templates (templates/*.template and templates/index.tsv) from a folder of
document settings templates in the older text form.

Usage: python3 -I convert_templates.py <template folder> <out folder>

The templates are read from outside the repository. Each one is text: comment lines start with "|", the first of
them giving the template's name (the file name it was installed under, ending in ".dxt"); then a header line; then
"Key: Value" settings; then "Styles:" and the styles (a name line and two indented lines of begin and end codes).

What is written: the same settings and styles, line for line, under BrailleCraft's own header line, with the name
line, the copyright lines and the rights lines left out (the name goes into the index). Other comments are kept.
Each template's file name is its name in small letters with every run of other characters turned into "-".
index.tsv lists, per template: the short code of the file it came from, the BrailleCraft file name, and the name.
"""
import os
import re
import sys

HEADER = "-braillecraft-template 1"


def slug(name):
    return re.sub(r"[^a-z0-9]+", "-", name.lower()).strip("-")


def template_name(first_line):
    """'| English (UEB) - basic.dxt' (sometimes with a date after it, or without the ending) -> the name."""
    text = first_line[1:].strip()
    m = re.match(r"(.*?)\.dxt\b", text, re.IGNORECASE)
    return (m.group(1) if m else text).strip()


def main(src, out):
    rows = []
    for f in sorted(os.listdir(src)):
        if not f.lower().endswith(".dxt"):
            continue
        lines = open(os.path.join(src, f), encoding="latin-1").read().replace("\r\n", "\n").split("\n")
        if not lines or not lines[0].startswith("|"):
            raise SystemExit(f"{f}: no name line")
        name = template_name(lines[0])
        body = []
        for line in lines[1:]:
            if line.startswith("|") and re.search(r"copyright|all rights? reserved", line, re.IGNORECASE):
                continue
            if line.startswith("-") and line.strip().endswith("dxt 1"):
                body.append(HEADER)
                continue
            body.append(line)
        if HEADER not in body:
            raise SystemExit(f"{f}: no header line")
        file = slug(name)
        with open(os.path.join(out, file + ".template"), "w", encoding="utf-8", newline="\n") as w:
            w.write("\n".join(body).rstrip("\n") + "\n")
        rows.append((f[:-4].lower(), file, name))
    names = [r[1] for r in rows]
    if len(set(names)) != len(names):
        raise SystemExit("two templates give the same file name")
    with open(os.path.join(out, "index.tsv"), "w", encoding="utf-8", newline="\n") as w:
        w.write("# Document templates: the short code of the template a document file names, the template file in this\n")
        w.write("# folder (without .template) and the template's name as File, New lists it.\n")
        w.write("# code\tfile\tname\n")
        for r in rows:
            w.write("\t".join(r) + "\n")
    print(f"{len(rows)} templates")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
