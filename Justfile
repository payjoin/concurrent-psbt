system := `nix eval --offline --raw --impure --expr builtins.currentSystem`

# Quick checks (fast dev cycle feedback)
check:
    nix build --no-update-lock-file '.#checks.{{ system }}.quick'

# All flake checks
check-all:
    nix flake check --no-update-lock-file

# Lint checks (formatting + source invariants)
lint:
    nix build --no-update-lock-file '.#checks.{{ system }}.lint'
