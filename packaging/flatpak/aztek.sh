#!/bin/sh
# Starts Aztek inside the Flatpak with the libraries and Python that came in it.
export PYTHONHOME=/app/usr
export PYTHONNOUSERSITE=1
export LD_LIBRARY_PATH="/app/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec /app/usr/bin/aztek "$@"
