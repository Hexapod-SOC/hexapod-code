{ pkgs ? import <nixpkgs> {} }:

pkgs.mkShell {
  buildInputs = [
    pkgs.python311
    pkgs.python311Packages.fastapi
    pkgs.python311Packages.uvicorn
    pkgs.python311Packages.requests
    pkgs.python311Packages.numpy
    pkgs.python311Packages.openai
    pkgs.python311Packages.networkx
    pkgs.python311Packages.pydantic
  ];
}
