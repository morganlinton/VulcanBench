// Hidden fail-to-pass tests: HTTP QUERY method support (RFC 10008).
// Test-only sub-package: references the new Router.Query API, so it does not
// compile at the base commit while the pass-to-pass guards still do.
package vbquery

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
)

func do(t *testing.T, h http.Handler, method, path string, body string) (*http.Response, string) {
	t.Helper()
	var rdr io.Reader
	if body != "" {
		rdr = strings.NewReader(body)
	}
	req := httptest.NewRequest(method, path, rdr)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	res := rec.Result()
	b, _ := io.ReadAll(res.Body)
	return res, string(b)
}

func TestVBQueryRouteMatches(t *testing.T) {
	r := chi.NewRouter()
	r.Query("/search", func(w http.ResponseWriter, req *http.Request) {
		b, _ := io.ReadAll(req.Body)
		w.Write([]byte("query: " + string(b)))
	})
	res, body := do(t, r, "QUERY", "/search", "select 1")
	if res.StatusCode != http.StatusOK || body != "query: select 1" {
		t.Fatalf("QUERY route: %d %q", res.StatusCode, body)
	}
}

func TestVBQueryViaMethodFunc(t *testing.T) {
	r := chi.NewRouter()
	r.MethodFunc("QUERY", "/reports", func(w http.ResponseWriter, req *http.Request) {
		w.Write([]byte("reports"))
	})
	res, body := do(t, r, "QUERY", "/reports", "")
	if res.StatusCode != http.StatusOK || body != "reports" {
		t.Fatalf("MethodFunc QUERY: %d %q", res.StatusCode, body)
	}
}

func TestVBQueryDistinctFromGet(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/search", func(w http.ResponseWriter, req *http.Request) {
		w.Write([]byte("get"))
	})
	r.Query("/search", func(w http.ResponseWriter, req *http.Request) {
		w.Write([]byte("query"))
	})
	if _, body := do(t, r, http.MethodGet, "/search", ""); body != "get" {
		t.Fatalf("GET must hit the GET handler, got %q", body)
	}
	if _, body := do(t, r, "QUERY", "/search", ""); body != "query" {
		t.Fatalf("QUERY must hit the QUERY handler, got %q", body)
	}
}

func TestVBQueryInAllowHeaderOn405(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/thing", func(w http.ResponseWriter, req *http.Request) {})
	r.Query("/thing", func(w http.ResponseWriter, req *http.Request) {})
	res, _ := do(t, r, http.MethodPost, "/thing", "")
	if res.StatusCode != http.StatusMethodNotAllowed {
		t.Fatalf("unregistered method must 405, got %d", res.StatusCode)
	}
	allow := strings.Join(res.Header.Values("Allow"), ",")
	if !strings.Contains(allow, "GET") || !strings.Contains(allow, "QUERY") {
		t.Fatalf("Allow must list GET and QUERY, got %q", allow)
	}
}
