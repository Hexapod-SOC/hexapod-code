{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.python311
    pkgs.python311Packages.pip
    pkgs.python311Packages.fastapi
    pkgs.python311Packages.uvicorn
    pkgs.python311Packages.requests
    pkgs.python311Packages.numpy
    pkgs.python311Packages.openai
    pkgs.python311Packages.networkx
    pkgs.python311Packages.pydantic
    pkgs.python311Packages.pydantic-core
  ];

  shellHook = ''
    if [ ! -d ".nix-venv" ]; then
      echo "Creating local .nix-venv for Python packages..."
      python -m venv .nix-venv
    fi

    export VIRTUAL_ENV="$PWD/.nix-venv"
    export PATH="$VIRTUAL_ENV/bin:$PATH"

    if ! python -c "import pvporcupine" >/dev/null 2>&1; then
      echo "Installing pvporcupine into .nix-venv..."
      python -m pip install pvporcupine
    fi
  '';
}
