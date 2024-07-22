#!/usr/bin/env python
"""Script to build all grammars"""

# pylint: disable-next=unused-import
import readline
import cmd
import os
import sys
import logging
from pathlib import Path
from platform import system
from shutil import copy
import subprocess

ci = os.getenv("GITHUB_ACTIONS")

logger = logging.getLogger(__name__)

# GitHub Actions log level names
logging.addLevelName(logging.ERROR, "error")
# logging.addLevelName(logging.INFO, "notice")
logging.addLevelName(logging.WARN, "warning")
logging.addLevelName(logging.DEBUG, "debug")

if ci is not None:
    logging.basicConfig(
        stream=sys.stdout,
        level=logging.DEBUG,
        format="::%(levelname)s title=make.py::%(message)s",
    )
else:
    logging.basicConfig(
        stream=sys.stderr,
        level=logging.DEBUG,
        format="%(asctime)s %(levelname)s %(message)s",
    )


cwd = Path.cwd().resolve()
logger.info("cwd: %s", cwd)


def lib_suffix():
    """Get appropriate dynamic library suffix for OS type"""
    match system():
        case "Windows":
            return "dll"
        case "Linux":
            return "so"
        case "Darwin":
            return "dylib"
        case _:
            return ""


def run(command: list[str], workdir: Path, err: str):
    logger.debug('workdir: %s', workdir)
    logger.debug('command: %s', command)
    # pylint: disable-next=exec-used
    proc = subprocess.run(
        command,
        capture_output=True,
        check=False,
        cwd=workdir,
    )
    for line in proc.stdout.splitlines():
        logging.info(line.decode())
    for line in proc.stderr.splitlines():
        logging.info(line.decode())
    if proc.returncode != 0:
        logging.error(err)
        return False
    return True


def _ts_build(grammar: Path, grammar_name: str, output: Path):
    if (
        run(
            command=[
                "tree-sitter",
                "generate",
                "--no-bindings",
            ],
            workdir=grammar.resolve(),
            err=f'Failed to execute "tree-sitter generate" for {grammar}',
        )
        is False
    ):
        return False
    if (
        run(
            command=[
                "tree-sitter",
                "build",
                "--output",
                output.joinpath(f"lib{grammar_name}.{lib_suffix()}"),
                ".",
            ],
            workdir=grammar,
            err=f'Failed to execute "tree-sitter build" for {grammar}',
        )
        is False
    ):
        return False
    return True


def build(output: Path, grammars: list[Path]):
    """Build entrypoint"""
    if len(grammars) == 0:
        grammars = sorted(cwd.joinpath("grammars").iterdir())

    for grammar in grammars:
        if grammar.is_dir() is False:
            logger.info("skipping path: %s", grammar)
            continue

        grammar_name = grammar.name
        if ci is not None:
            print(f"::group::Build {grammar_name}")
        else:
            print("---")
        logger.info("building grammar: %s", grammar_name)

        # # Skip built grammars
        # if next(grammar.glob(f"**/libtree-sitter-*.{lib_suffix()}"), False) is False:
        #     continue

        match grammar_name:
            case "tree-sitter-adl":
                logging.info("skip building: bad licence")
                continue
            case "tree-sitter-angular":
                logging.info("skip building: bad licence")
                continue
            case (
                "tree-sitter-glimmer"
            ):  # https://github.com/ember-tooling/tree-sitter-glimmer/issues/139
                logging.info("skip building: bad licence")
                continue
            case "tree-sitter-odin":
                logging.info("skip building: unknown issue")
                continue
            case "tree-sitter-rcl":  # monorepo
                grammar = grammar.joinpath("grammar").joinpath("tree-sitter-rcl")
            case "tree-sitter-php":  # multi-grammar
                grammar = grammar.joinpath("php")

        def _build_multi(dirs: list):
            for subdir in dirs:
                # pylint: disable-next=cell-var-from-loop
                if _ts_build(grammar.joinpath(subdir), grammar_name, output) is False:
                    continue

        match grammar_name:
            case "tree-sitter-markdown":
                _build_multi(["tree-sitter-markdown", "tree-sitter-markdown-inline"])
            case "tree-sitter-ocaml":
                _build_multi(["grammars/ocaml", "grammars/interface", "grammars/type"])
            case "tree-sitter-typescript":
                _build_multi(["tsx", "typescript"])
            case "tree-sitter-wasm":
                _build_multi(["wast", "wat"])
            case _:
                if _ts_build(grammar, grammar_name, output) is False:
                    continue

        def copy_lic(src_lic_path: Path):
            logging.info("copying '%s'", src_lic_path)
            # pylint: disable-next=cell-var-from-loop
            copy(src_lic_path, output.joinpath(f"{grammar_name}.LICENSE"))

        match grammar_name:
            case "tree-sitter-dhall":
                copy_lic(grammar.joinpath("LICENSE"))
            case "tree-sitter-rcl":
                copy_lic(grammar.joinpath("..", "..", "LICENSE").resolve())
            case "tree-sitter-ron":
                copy_lic(grammar.joinpath("LICENSE-APACHE"))
            case "tree-sitter-slint":
                copy_lic(grammar.joinpath("LICENSES/MIT.txt"))
            case _:
                # Grab all LICENSE files
                licg = grammar.glob("LICENSE*")
                copg = grammar.glob("COPYING*")
                # Get first one
                lic = next(copg, next(licg, ""))

                if lic != "" and lic.exists() is True:
                    suffix = "LICENSE"
                    if lic.name.startswith("COPYING"):
                        suffix = "COPYING"
                    logging.info("copying '%s'", lic)
                    copy(lic, output.joinpath(f"{grammar_name}.{suffix}"))
                else:
                    logging.error("No licence found!!!")

        if ci is not None:
            print("\n::endgroup::")


# pylint: disable=missing-class-docstring,missing-function-docstring,invalid-name
class TreeSitterMake(cmd.Cmd):
    intro = "tree-sitter-grammars shell:   type help or ? to list commands.\n"
    prompt = "(ts-grammars) "

    def do_build(self, arg):
        paths = []
        for path in parse(arg):
            paths.append(Path(path))
        build(output_dir(), paths)

    def do_return(self, _arg):
        return True

    def do_quit(self, _arg):
        return True

    def do_exit(self, _arg):
        return True

    def do_EOF(self, _arg):
        return True


def parse(arg):
    "Make args into tuple"
    return tuple(map(str, arg.split()))


def output_dir():
    "Get artefact output dir"
    output = cwd.joinpath("output")
    logging.info("output dir: %s", output)
    if output.exists() is False:
        logging.info("Creating 'output' dir")
        output.mkdir(mode=0o755, parents=True, exist_ok=True)
    return output


if __name__ == "__main__":
    make = TreeSitterMake()
    if len(sys.argv) == 1:
        make.cmdqueue = ["build", "return"]
    make.cmdloop()
