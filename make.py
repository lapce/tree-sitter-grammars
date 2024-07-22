#!/usr/bin/env python
"""Script to build all grammars"""

# pylint: disable=missing-class-docstring,missing-function-docstring,invalid-name

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
logging.addLevelName(logging.INFO, "info")
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
    logger.debug("workdir: %s", workdir)
    logger.debug("command: %s", command)

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


def ts_build(grammar: Path, grammar_name: str, output: Path, generate=True, npm=False):
    if npm is True:
        command = ["npm", "install"]
        if (
            run(
                command=command,
                workdir=grammar.resolve(),
                err=f"Failed to execute {command} for {grammar}",
            )
            is False
        ):
            return False
    if generate is True:
        command = ["tree-sitter", "generate", "--no-bindings"]
        if (
            run(
                command=command,
                workdir=grammar.resolve(),
                err=f"Failed to execute {command} for {grammar}",
            )
            is False
        ):
            return False

    command = [
        "tree-sitter",
        "build",
        "--output",
        output.joinpath(f"lib{grammar_name}.{lib_suffix()}"),
        ".",
    ]
    if (
        run(
            command=command,
            workdir=grammar,
            err=f"Failed to execute {command} for {grammar}",
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

        # pylint: disable=cell-var-from-loop
        def _symlink_module(mod: str):
            if grammar.joinpath("node_modules").resolve().exists() is False:
                os.mkdir(grammar.joinpath("node_modules").resolve())
            os.symlink(
                grammar.joinpath("..", mod).resolve(),
                grammar.joinpath("node_modules", mod).resolve(),
            )

        # Prep phase

        match grammar_name:
            case "tree-sitter-adl":
                logging.info("skip building: bad licence")
                continue
            case "tree-sitter-angular":
                logging.info("skip building: bad licence")
                continue
            # case "tree-sitter-astro":
            #     _symlink_module("tree-sitter-html")
            # case "tree-sitter-cpp":
            #     _symlink_module("tree-sitter-c")
            case (
                "tree-sitter-glimmer"
            ):  # https://github.com/ember-tooling/tree-sitter-glimmer/issues/139
                logging.info("skip building: bad licence")
                continue
            # case "tree-sitter-glsl":
            #     _symlink_module("tree-sitter-c")
            case "tree-sitter-odin":
                logging.info("skip building: unknown issue")
                continue
            case "tree-sitter-rcl":  # monorepo
                grammar = grammar.joinpath("grammar").joinpath("tree-sitter-rcl")
            case "tree-sitter-php":  # multi-grammar
                grammar = grammar.joinpath("php")

        def _build_multi(dirs: list, generate=False, npm=False):
            for subdir in dirs:
                # pylint: disable-next=cell-var-from-loop
                if ts_build(grammar.joinpath(subdir), grammar_name, output, generate, npm) is False:
                    continue

        # Build phase

        match grammar_name:
            case "tree-sitter-astro":
                if ts_build(grammar, grammar_name, output, npm=True) is False:
                    continue
            case "tree-sitter-cpp":
                if ts_build(grammar, grammar_name, output, npm=True) is False:
                    continue
            case "tree-sitter-c-sharp":
                if ts_build(grammar, grammar_name, output, generate=False) is False:
                    continue
            case "tree-sitter-glsl":
                if ts_build(grammar, grammar_name, output, npm=True) is False:
                    continue
            case "tree-sitter-markdown":
                _build_multi(["tree-sitter-markdown", "tree-sitter-markdown-inline"])
            case "tree-sitter-ocaml":
                _build_multi(["grammars/ocaml", "grammars/interface", "grammars/type"])
            case "tree-sitter-typescript":
                _build_multi(["tsx", "typescript"], npm=True)
            case "tree-sitter-wasm":
                _build_multi(["wast", "wat"])
            case _:
                if ts_build(grammar, grammar_name, output) is False:
                    continue

        # License phase

        def _copy_lic(src_lic_path: Path):
            logging.info("copying '%s'", src_lic_path)
            # pylint: disable-next=cell-var-from-loop
            copy(src_lic_path, output.joinpath(f"{grammar_name}.LICENSE"))

        match grammar_name:
            case "tree-sitter-dhall":
                _copy_lic(grammar.joinpath("LICENSE"))
            case "tree-sitter-rcl":
                _copy_lic(grammar.joinpath("..", "..", "LICENSE").resolve())
            case "tree-sitter-ron":
                _copy_lic(grammar.joinpath("LICENSE-APACHE"))
            case "tree-sitter-slint":
                _copy_lic(grammar.joinpath("LICENSES", "MIT.txt"))
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
                    logging.error("%s: No licence found!!!", grammar_name)

        if ci is not None:
            print("\n::endgroup::")


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
