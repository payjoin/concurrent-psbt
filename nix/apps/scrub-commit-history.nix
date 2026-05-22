{
  perSystem =
    { pkgs, ... }:
    let
      scrub-commit-history = pkgs.writeShellApplication {
        name = "scrub-commit-history";
        runtimeInputs = with pkgs; [
          git
          jujutsu
          nix-output-monitor
        ];
        text = builtins.readFile ../../contrib/scrub-commit-history.sh;
      };
    in
    {
      packages.scrub-commit-history = scrub-commit-history;

      apps.scrub-commit-history = {
        type = "app";
        program = "${scrub-commit-history}/bin/scrub-commit-history";
        meta.description = "Check commit history for hygiene and flake check regressions";
      };
    };
}
