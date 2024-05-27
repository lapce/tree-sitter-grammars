# tree-sitter-grammars

## Grammars

### Checkout

```shell
git submodule update --init
```

### Checkout recursively (for tests and etc.)

```shell
git submodule update --init --recursive
```

### Build

```shell
./make.py
```

Built artefacts are in `./output`

## Git modules

Helper tool is used only to obtain all remotes and pretty-print sorted `.gitmodules` config. If you need to build the repo, it's designed this way to work with just `git` CLI alone.

### Format

Git does not support more than one URL per submodule BUT it also doesn't care if you put more of them (technically it does in certain actions, but it just continues to do its job).
Based on that, all remotes that need to be configured per submodule are added as `url` key, e.g.

```gitconfig
[submodule "grammars/tree-sitter-bash"]
	branch = "lapce/0.4.0"
	path = "grammars/tree-sitter-bash"
	url = "git@github.com:tree-sitter/tree-sitter-bash.git"
	url = "git@github.com:panekj/tree-sitter-bash.git"
```

Helper tool will create remotes based on that config.
Additionally first and last `url` key have special meaning, first `url` will become `upstream` remote and last `url` will become `origin` remote, e.g.:

```gitconfig
[submodule "grammars/tree-sitter-sql"]
	branch = "lapce/0.4.0"
	path = "grammars/tree-sitter-sql"
	url = "git@github.com:DerekStride/tree-sitter-sql.git" # upstream
	url = "git@github.com:m-novikov/tree-sitter-sql.git"
	url = "git@github.com:panekj/tree-sitter-sql.git"      # origin
```

#### Caveat

Helper tool will not preserve comments, since the obtained config from git2 lib does not present them.
