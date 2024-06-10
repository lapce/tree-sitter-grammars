#!/usr/bin/env bash
set -eu -o pipefail

root=$(pwd)
output="${root}/output"
mkdir -p "${output}"

cleanup() {
    cd $root
}

trap cleanup 1 2 3 6

pushd ./grammars

MAKE_CLEAN="${MAKE_CLEAN:=}"

for grammar in * ; do
    cd "${root}/grammars/${grammar}"

    case "${grammar}" in
        "tree-sitter-adl") continue ;; # bad licence
        "tree-sitter-angular") continue ;; # bad licence
        "tree-sitter-odin") continue ;; # unknown issue
        "tree-sitter-rcl") cd ./grammar/tree-sitter-rcl ;; # monorepo
        "tree-sitter-tcl") continue ;; # broken grammar
        *) ;;
    esac

    ${MAKE_CLEAN}
    make

    case "${grammar}" in
        "tree-sitter-markdown")
            cp ./tree-sitter-markdown{,-inline}/lib"${grammar}".* "${output}"/
        ;;
        "tree-sitter-php")
            cp ./tree-sitter-php/lib"${grammar}".* "${output}"/
        ;;
        "tree-sitter-typescript")
            cp ./{tsx,typescript}/lib"${grammar}".* "${output}"/
        ;;
        "tree-sitter-wasm")
            cp ./wa{,s}t/lib"${grammar}".* "${output}"/
        ;;
        *)
            cp ./lib"${grammar}".* "${output}"/
        ;;
    esac

    case "${grammar}" in
        "tree-sitter-dhall")
            cp LICENSE "${output}"/"${grammar}".LICENSE
        ;;
        "tree-sitter-ron")
            cp LICENSE-APACHE "${output}"/"${grammar}".LICENSE
        ;;
        "tree-sitter-slint")
            cp LICENSES/MIT.txt "${output}"/"${grammar}".LICENSE
        ;;
        *)
            if [ -e LICENSE* ]; then
                cp LICENSE* "${output}"/"${grammar}".LICENSE
            elif [ -e COPYING* ]; then
                cp COPYING* "${output}"/"${grammar}".COPYING
            fi
        ;;
    esac
done

popd
