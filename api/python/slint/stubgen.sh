export PATH="$HOME/.pyenv/bin:$PATH";
eval "$(pyenv init -)";
pyenv local 3.13.1;
export PYO3_PYTHON="$(pyenv which python3)";
export LIBRARY_PATH="$HOME/.pyenv/versions/3.13.1/lib";
export DYLD_LIBRARY_PATH="$HOME/.pyenv/versions/3.13.1/lib";
export RUSTFLAGS="-C link-arg=-L$HOME/.pyenv/versions/3.13.1/lib -C link-arg=-lpython3.13";
cargo run -pslint-python --bin stub-gen --features stubgen,
