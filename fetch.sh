#!/bin/sh

set -eux

ID=$1

curl --header "Accept:application/json" https://hydra.nixos.org/eval/$ID/builds > latest-eval-builds
