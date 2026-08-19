<?php

/**
 * A PSR-4 autoloader for using this library without Composer.
 *
 * The package has no dependencies, so dropping the directory somewhere and
 * requiring this file is a complete installation:
 *
 *     require __DIR__ . '/json-compress/autoload.php';
 *
 *     use TechStyle\JsonCompress\JsonCompress;
 *
 * Composer users need none of this - `composer require tech-style/json-compress`
 * wires the same classes into the usual autoloader.
 */

declare(strict_types=1);

spl_autoload_register(static function (string $class): void {
    $prefix = 'TechStyle\\JsonCompress\\';
    if (!str_starts_with($class, $prefix)) {
        return;
    }

    $file = __DIR__ . '/src/' . str_replace('\\', '/', substr($class, strlen($prefix))) . '.php';
    if (is_file($file)) {
        require $file;
    }
});
