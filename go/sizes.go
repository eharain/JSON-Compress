package jsoncompress

import (
	"bytes"
	"compress/gzip"
	"io"
)

// Byte measurement and gzip.
//
// What actually goes over the wire is UTF-8 bytes, so that is what these count.
// The gzip helpers are here because the interesting question about any
// compression scheme is what it is still worth once gzip has had its turn, and
// answering it should not need a second import at the call site.

// ByteLength is the length of a string in UTF-8 bytes - what goes over the wire,
// as opposed to the number of characters in it.
func ByteLength(text string) int {
	return len(text)
}

// GzipBytes gzips a document.
//
// The header carries no timestamp, so the same input always gives the same bytes
// and a size can be compared across runs. The level is zlib's own default, which
// is what browsers use for CompressionStream, so a figure measured here is
// comparable with one measured in the JavaScript implementation.
func GzipBytes(data []byte) ([]byte, error) {
	var out bytes.Buffer
	writer, err := gzip.NewWriterLevel(&out, gzip.DefaultCompression)
	if err != nil {
		return nil, err
	}
	if _, err := writer.Write(data); err != nil {
		return nil, err
	}
	if err := writer.Close(); err != nil {
		return nil, err
	}
	return out.Bytes(), nil
}

// Gunzip reverses [GzipBytes].
func Gunzip(data []byte) ([]byte, error) {
	reader, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, err
	}
	defer reader.Close()
	return io.ReadAll(reader)
}

// GzipSize is how many bytes a document takes once gzipped. Compression cannot
// fail on a byte slice, so this reports the size and no error; it is the figure
// [Report] carries.
func GzipSize(data []byte) int {
	out, err := GzipBytes(data)
	if err != nil {
		return 0
	}
	return len(out)
}
