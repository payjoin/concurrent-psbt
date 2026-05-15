{ inputs, ... }:
{
  perSystem =
    { pkgs, ... }:
    let
      src = inputs.self;
      checks = {
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
          ];
        };
      };
    };
}
