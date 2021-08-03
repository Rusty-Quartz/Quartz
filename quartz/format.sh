# `cargo fmt` ignores some directories in here for some reason, so this file just invokes `rustfmt`
# with the appropriate config on all rust source files manually

find -name "*.rs" -exec rustfmt --config-path ../ {} \;
