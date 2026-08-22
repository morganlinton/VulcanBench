// Hidden pass-to-pass guards: existing routing and 405 behavior unchanged.
package vbreg

import (
	"io"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/go-chi/chi/v5"
)

func do(t *testing.T, h http.Handler, method, path string) (*http.Response, string) {
	t.Helper()
	req := httptest.NewRequest(method, path, nil)
	rec := httptest.NewRecorder()
	h.ServeHTTP(rec, req)
	res := rec.Result()
	b, _ := io.ReadAll(res.Body)
	return res, string(b)
}

func TestVBGetAndPostRouting(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/x", func(w http.ResponseWriter, req *http.Request) { w.Write([]byte("get")) })
	r.Post("/x", func(w http.ResponseWriter, req *http.Request) { w.Write([]byte("post")) })
	if _, body := do(t, r, http.MethodGet, "/x"); body != "get" {
		t.Fatalf("GET broken: %q", body)
	}
	if _, body := do(t, r, http.MethodPost, "/x"); body != "post" {
		t.Fatalf("POST broken: %q", body)
	}
}

func TestVBUrlParams(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/users/{id}", func(w http.ResponseWriter, req *http.Request) {
		w.Write([]byte(chi.URLParam(req, "id")))
	})
	if _, body := do(t, r, http.MethodGet, "/users/42"); body != "42" {
		t.Fatalf("URL param broken: %q", body)
	}
}

func TestVB405WithAllowHeader(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/only", func(w http.ResponseWriter, req *http.Request) {})
	res, _ := do(t, r, http.MethodDelete, "/only")
	if res.StatusCode != http.StatusMethodNotAllowed {
		t.Fatalf("want 405, got %d", res.StatusCode)
	}
	allow := strings.Join(res.Header.Values("Allow"), ",")
	if !strings.Contains(allow, "GET") {
		t.Fatalf("Allow must list GET, got %q", allow)
	}
}

func TestVB404ForUnknownRoute(t *testing.T) {
	r := chi.NewRouter()
	r.Get("/known", func(w http.ResponseWriter, req *http.Request) {})
	res, _ := do(t, r, http.MethodGet, "/unknown")
	if res.StatusCode != http.StatusNotFound {
		t.Fatalf("want 404, got %d", res.StatusCode)
	}
}
