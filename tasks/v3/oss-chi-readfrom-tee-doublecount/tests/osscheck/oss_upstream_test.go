// Hidden test for oss-chi-readfrom-tee-doublecount.
//
// When a WrapResponseWriter has a Tee writer set and the response body is sent
// via io.ReaderFrom (e.g. http.ServeContent / io.Copy fast path), BytesWritten
// must report the actual number of body bytes once, the tee writer must receive
// a copy, and the underlying writer must receive the body. Expected behavior
// captured from the upstream fix (go-chi/chi PR #1085, issue #1067).
package osscheck

import (
	"bufio"
	"bytes"
	"errors"
	"io"
	"net"
	"net/http"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5/middleware"
)

// readFromRecorder implements the interfaces (Flusher, Hijacker, ReaderFrom)
// that make chi select its io.ReaderFrom-capable wrapper for HTTP/1.x.
type readFromRecorder struct {
	header http.Header
	buf    bytes.Buffer
	code   int
}

func (r *readFromRecorder) Header() http.Header         { return r.header }
func (r *readFromRecorder) WriteHeader(code int)        { r.code = code }
func (r *readFromRecorder) Write(p []byte) (int, error) { return r.buf.Write(p) }
func (r *readFromRecorder) Flush()                      {}
func (r *readFromRecorder) Hijack() (net.Conn, *bufio.ReadWriter, error) {
	return nil, nil, errors.New("not supported")
}
func (r *readFromRecorder) ReadFrom(rr io.Reader) (int64, error) {
	return io.Copy(&r.buf, rr)
}

func TestOSSReadFromWithTeeCountsBytesOnce(t *testing.T) {
	rec := &readFromRecorder{header: make(http.Header)}
	w := middleware.NewWrapResponseWriter(rec, 1)

	var tee bytes.Buffer
	w.Tee(&tee)

	rf, ok := w.(io.ReaderFrom)
	if !ok {
		t.Fatalf("wrapped writer does not implement io.ReaderFrom")
	}

	const input = "hello world"
	n, err := rf.ReadFrom(strings.NewReader(input))
	if err != nil {
		t.Fatalf("ReadFrom error: %v", err)
	}
	if n != int64(len(input)) {
		t.Fatalf("ReadFrom returned n=%d, want %d", n, len(input))
	}
	if got := w.BytesWritten(); got != len(input) {
		t.Fatalf("BytesWritten()=%d, want %d (bytes must not be double-counted)", got, len(input))
	}
	if tee.String() != input {
		t.Fatalf("tee writer got %q, want %q", tee.String(), input)
	}
	if rec.buf.String() != input {
		t.Fatalf("underlying writer got %q, want %q", rec.buf.String(), input)
	}
}

func TestOSSReadFromWithoutTeeStable(t *testing.T) {
	rec := &readFromRecorder{header: make(http.Header)}
	w := middleware.NewWrapResponseWriter(rec, 1)

	rf, ok := w.(io.ReaderFrom)
	if !ok {
		t.Fatalf("wrapped writer does not implement io.ReaderFrom")
	}

	const input = "plain body"
	if _, err := rf.ReadFrom(strings.NewReader(input)); err != nil {
		t.Fatalf("ReadFrom error: %v", err)
	}
	if rec.buf.String() != input {
		t.Fatalf("underlying writer got %q, want %q", rec.buf.String(), input)
	}
}

// Additional fail_to_pass coverage for the same double-count bug: at the base,
// ReadFrom with a Tee adds n to basicWriter.bytes *after* io.Copy already
// counted it via basicWriter.Write, so BytesWritten reports twice the body size.

func TestOSSReadFromWithTeeTwoCallsAccumulateOnce(t *testing.T) {
	rec := &readFromRecorder{header: make(http.Header)}
	w := middleware.NewWrapResponseWriter(rec, 1)

	var tee bytes.Buffer
	w.Tee(&tee)

	rf, ok := w.(io.ReaderFrom)
	if !ok {
		t.Fatalf("wrapped writer does not implement io.ReaderFrom")
	}

	const first, second = "abc", "defg"
	if _, err := rf.ReadFrom(strings.NewReader(first)); err != nil {
		t.Fatalf("ReadFrom(first) error: %v", err)
	}
	if _, err := rf.ReadFrom(strings.NewReader(second)); err != nil {
		t.Fatalf("ReadFrom(second) error: %v", err)
	}

	want := len(first) + len(second)
	if got := w.BytesWritten(); got != want {
		t.Fatalf("BytesWritten()=%d, want %d (bytes must not be double-counted)", got, want)
	}
	if tee.String() != first+second {
		t.Fatalf("tee writer got %q, want %q", tee.String(), first+second)
	}
}

func TestOSSWriteThenReadFromWithTeeCountsOnce(t *testing.T) {
	rec := &readFromRecorder{header: make(http.Header)}
	w := middleware.NewWrapResponseWriter(rec, 1)

	var tee bytes.Buffer
	w.Tee(&tee)

	const head, body = "xy", "hello"
	if _, err := w.Write([]byte(head)); err != nil {
		t.Fatalf("Write error: %v", err)
	}

	rf, ok := w.(io.ReaderFrom)
	if !ok {
		t.Fatalf("wrapped writer does not implement io.ReaderFrom")
	}
	if _, err := rf.ReadFrom(strings.NewReader(body)); err != nil {
		t.Fatalf("ReadFrom error: %v", err)
	}

	want := len(head) + len(body)
	if got := w.BytesWritten(); got != want {
		t.Fatalf("BytesWritten()=%d, want %d (bytes must not be double-counted)", got, want)
	}
	if rec.buf.String() != head+body {
		t.Fatalf("underlying writer got %q, want %q", rec.buf.String(), head+body)
	}
}
