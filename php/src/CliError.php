<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress;

use RuntimeException;

/**
 * Something the person at the terminal needs to be told about.
 *
 * Caught by {@see Cli::main()}, printed to standard error without a stack
 * trace, and turned into exit code 1. It carries no cause: by the time one of
 * these is thrown, the message has already been written for a reader rather
 * than for a debugger.
 *
 * @internal
 */
final class CliError extends RuntimeException
{
}
