<?php

declare(strict_types=1);

namespace TechStyle\JsonCompress\Tests;

use PHPUnit\Framework\Attributes\CoversClass;
use PHPUnit\Framework\TestCase;
use TechStyle\JsonCompress\Tokens;

/**
 * Token tests.
 *
 * Both alphabets have to be bijective over the whole range they are used on: a
 * token that decoded to a different index, or two indices that spelled the same
 * token, would silently rename a key.
 */
#[CoversClass(Tokens::class)]
final class TokensTest extends TestCase
{
    public function testBase62StartsWhereTheSpecificationSaysItDoes(): void
    {
        $this->assertSame('0', Tokens::toBase62(0));
        $this->assertSame('9', Tokens::toBase62(9));
        $this->assertSame('A', Tokens::toBase62(10));
        $this->assertSame('z', Tokens::toBase62(61));
        $this->assertSame('10', Tokens::toBase62(62));
    }

    public function testBase62RoundTripsOverItsFirstThreeBlocks(): void
    {
        // The loop counts rather than asserts, so that 20,000 indices cost one
        // assertion rather than 20,000 of them.
        $wrong = [];
        for ($n = 0; $n < 20000; $n++) {
            if (Tokens::fromBase62(Tokens::toBase62($n)) !== $n) {
                $wrong[] = $n;
            }
        }

        $this->assertSame([], $wrong, 'indices that did not survive the round trip');
    }

    public function testBase62LengthAgreesWithTheTokenItDescribes(): void
    {
        foreach ([0, 61, 62, 3843, 3844, 238327, 238328, 14776335, 14776336] as $n) {
            $this->assertSame(strlen(Tokens::toBase62($n)), Tokens::base62Length($n), (string) $n);
        }
    }

    public function testBase62RejectsWhatIsNotABase62Token(): void
    {
        $this->assertSame(-1, Tokens::fromBase62(''));
        $this->assertSame(-1, Tokens::fromBase62('~'));
        $this->assertSame(-1, Tokens::fromBase62('a b'));
    }

    public function testKeyTokensAlwaysStartWithALetter(): void
    {
        $this->assertSame('A', Tokens::toKeyToken(0));
        $this->assertSame('z', Tokens::toKeyToken(51));
        $this->assertSame('A0', Tokens::toKeyToken(52));
        $this->assertSame('Az', Tokens::toKeyToken(113));
        $this->assertSame('B0', Tokens::toKeyToken(114));

        $wrong = [];
        for ($n = 0; $n < 5000; $n++) {
            if (preg_match('/^[A-Za-z][0-9A-Za-z]*$/', Tokens::toKeyToken($n)) !== 1) {
                $wrong[] = $n;
            }
        }

        $this->assertSame([], $wrong, 'indices whose token was not letter-first');
    }

    public function testKeyTokensAreBijective(): void
    {
        // Past the end of the three-character block, so every block boundary the
        // format defines is crossed.
        $collisions = [];
        $wrong = [];
        $seen = [];

        for ($n = 0; $n < 210000; $n++) {
            $token = Tokens::toKeyToken($n);
            if (isset($seen[$token])) {
                $collisions[] = $token;
            }
            $seen[$token] = $n;
            if (Tokens::fromKeyToken($token) !== $n) {
                $wrong[] = $n;
            }
        }

        $this->assertSame([], $collisions, 'two indices spelled the same token');
        $this->assertSame([], $wrong, 'indices that did not survive the round trip');
    }

    public function testKeyTokenBlocksAreTheSizeTheSpecificationSays(): void
    {
        // 52 one-character tokens, then 52 * 62, then 52 * 62 * 62.
        $this->assertSame(1, strlen(Tokens::toKeyToken(51)));
        $this->assertSame(2, strlen(Tokens::toKeyToken(52)));
        $this->assertSame(2, strlen(Tokens::toKeyToken(3275)));
        $this->assertSame(3, strlen(Tokens::toKeyToken(3276)));
        $this->assertSame(3, strlen(Tokens::toKeyToken(203163)));
        $this->assertSame(4, strlen(Tokens::toKeyToken(203164)));
    }

    public function testKeyTokensRejectWhatIsNotOne(): void
    {
        $this->assertSame(-1, Tokens::fromKeyToken(''));
        $this->assertSame(-1, Tokens::fromKeyToken('0'));
        $this->assertSame(-1, Tokens::fromKeyToken('~t'));
        $this->assertSame(-1, Tokens::fromKeyToken('A~'));
    }
}
