{ inputs, ... }:
{
  perSystem =
    { pkgs, ... }:
    let
      rev = inputs.self.shortRev or "dirty";
      src = inputs.self;
      checks = {
        no-todo-comments = pkgs.runCommand "no-todo-comments-${rev}" { inherit src; } ''
          if grep -rn --exclude-dir=contrib 'TO[D]O\|FIX[M]E' $src/ 2>/dev/null; then
            echo "FAIL: unresolved work-item markers found"
            exit 1
          fi
          mkdir -p $out
        '';
      };
    in
    {
      checks = checks // {
        quick = pkgs.symlinkJoin {
          name = "quick-checks";
          paths = [
          ];
        };
        lint = pkgs.symlinkJoin {
          name = "lint-checks";
          paths = [
            checks.no-todo-comments
          ];
        };
      };
    };
}
