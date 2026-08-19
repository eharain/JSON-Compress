"""Allow `python -m json_compress` to run the command line."""

from .cli import main

raise SystemExit(main())
