default: build

# print formatted .gitmodules
print:
	cargo run -- print

# update submodules
update:
	cargo run -- update

# build all grammars
build:
	python ./make.py

# install tree-sitter-cli
tree-sitter:
	cargo install tree-sitter-cli@^0.22 --locked
