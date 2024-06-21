#!/usr/bin/env python
"""Script to build all grammars"""

import os
import sys
import logging
from pathlib import Path
from platform import system
from shutil import copy
from subprocess import run

ci = os.getenv("GITHUB_ACTIONS")

logger = logging.getLogger(__name__)
logging.basicConfig(stream=sys.stdout, level=logging.INFO, format='%(asctime)s %(message)s')

cwd = Path.cwd().resolve()
logger.info("cwd: %s", cwd)


def make_exec():
    """Get appropriate executable name for OS type"""
    match system():
        case "Windows":
            return "make.exe"
        case _:
            return "make"


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


def build(grammar, grammar_name):
    """Build tree-sitter grammar"""
    # pylint: disable-next=exec-used
    make = run(['tree-sitter', 'build'], capture_output=True, check=False, cwd=grammar)
    for line in make.stdout.splitlines():
        logging.info(line.decode())
    for line in make.stderr.splitlines():
        logging.info(line.decode())
    if make.returncode != 0:
        logging.error('Failed to execute "tree-sitter build" for %s', grammar_name)
        return False
    return True

def main():
    """Main program"""
    output = cwd.joinpath("output")
    logging.info("output dir: %s", output)
    if output.exists() is False:
        logging.info("Creating 'output' dir")
        output.mkdir(mode=0o755, parents=True, exist_ok=True)

    for grammar in sorted(cwd.joinpath("grammars").iterdir()):
        if grammar.is_dir() is False:
            logger.info("skipping path: %s", grammar)
            continue

        grammar_name = grammar.name
        if ci is not None:
            print(f"::group::Build {grammar_name}")
        logger.info("building grammar: %s", grammar_name)

        # # Skip built grammars
        # if next(grammar.glob(f"**/libtree-sitter-*.{lib_suffix()}"), False) is False:
        #     continue

        match grammar_name:
            case "tree-sitter-adl":  # bad licence
                continue
            case "tree-sitter-angular":  # bad licence
                continue
            case "tree-sitter-odin":  # unknown issue
                continue
            case "tree-sitter-rcl":  # monorepo
                grammar = grammar.joinpath("grammar").joinpath("tree-sitter-rcl")
            case "tree-sitter-php":  # multi-grammar
                grammar = grammar.joinpath("php")
            case "tree-sitter-vue":
                continue # C++ grammar
            case "tree-sitter-sql":
                continue # C++ grammar

        match grammar_name:
            case "tree-sitter-markdown":
                for subdir in ["tree-sitter-markdown", "tree-sitter-markdown-inline"]:
                    if build(grammar.joinpath(subdir), grammar_name) is False:
                        continue
            case "tree-sitter-ocaml":
                for subdir in ["grammars/ocaml", "grammars/interface", "grammars/type"]:
                    if build(grammar.joinpath(subdir), grammar_name) is False:
                        continue
            case "tree-sitter-typescript":
                for subdir in ["tsx", "typescript"]:
                    if build(grammar.joinpath(subdir), grammar_name) is False:
                        continue
            case "tree-sitter-wasm":
                for subdir in ["wast", "wat"]:
                    if build(grammar.joinpath(subdir), grammar_name) is False:
                        continue
            case _:
                if build(grammar, grammar_name) is False:
                    continue

        for lib in grammar.glob(f"**/libtree-sitter-*.{lib_suffix()}"):
            lib_path = grammar.joinpath(lib)
            logging.info("copying '%s' to output", lib_path)
            copy(lib_path, output)

        def copy_lic(src_lic_path, dst_lic_path):
            logging.info("copying '%s' to '%s'", src_lic_path, dst_lic_path)
            copy(src_lic_path, dst_lic_path)

        match grammar_name:
            case "tree-sitter-dhall":
                copy_lic(
                    grammar.joinpath("LICENSE"),
                    output.joinpath(
                        f"{grammar_name}.LICENSE",
                    ),
                )
            case "tree-sitter-ron":
                copy_lic(
                    grammar.joinpath("LICENSE-APACHE"),
                    output.joinpath(
                        f"{grammar_name}.LICENSE",
                    ),
                )
            case "tree-sitter-slint":
                copy_lic(
                    grammar.joinpath("LICENSES/MIT.txt"),
                    output.joinpath(
                        f"{grammar_name}.LICENSE",
                    ),
                )
            case _:
                # Grab all LICENSE files
                licg = grammar.glob("LICENSE*")
                copg = grammar.glob("COPYING*")
                # Get first one
                lic = next(copg, next(licg, ""))

                if lic != '' and lic.exists() is True:
                    suffix = "LICENSE"
                    if lic.name.startswith("COPYING"):
                        suffix = "COPYING"
                    copy_lic(lic, output.joinpath(f"{grammar_name}.{suffix}"))

        if ci is not None:
            print("\n::endgroup::")


if __name__ == "__main__":
    main()
