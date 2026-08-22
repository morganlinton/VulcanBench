// Hidden pass-to-pass guards: existing error handling and routing behavior
// must not change. Compiles and passes at the base commit.
package vbreg

import (
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	echo "github.com/labstack/echo/v5"
)

func TestVBDefaultErrorHandlerStillDefault(t *testing.T) {
	e := echo.New()
	e.GET("/boom", func(c *echo.Context) error {
		return &echo.HTTPError{Code: http.StatusTeapot, Message: "my_error"}
	})
	req := httptest.NewRequest(http.MethodGet, "/boom", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusTeapot {
		t.Fatalf("status = %d, want 418", rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "application/json") {
		t.Fatalf("default handler content type = %q, want application/json", ct)
	}
}

func TestVBBasicRoutingWorks(t *testing.T) {
	e := echo.New()
	e.GET("/hello/:name", func(c *echo.Context) error {
		return c.String(http.StatusOK, "hi "+c.Param("name"))
	})
	req := httptest.NewRequest(http.MethodGet, "/hello/vulcan", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK || rec.Body.String() != "hi vulcan" {
		t.Fatalf("routing broken: %d %q", rec.Code, rec.Body.String())
	}
}

func TestVBJSONResponseContentType(t *testing.T) {
	e := echo.New()
	e.GET("/j", func(c *echo.Context) error {
		return c.JSON(http.StatusOK, map[string]string{"ok": "yes"})
	})
	req := httptest.NewRequest(http.MethodGet, "/j", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); !strings.HasPrefix(ct, "application/json") {
		t.Fatalf("content type = %q, want application/json", ct)
	}
}
