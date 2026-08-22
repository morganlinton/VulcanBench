// Hidden fail-to-pass tests: a group's implicitly registered RouteNotFound
// routes must be overwritable even when the router forbids overwriting.
// Behavioral (no new API), so this compiles at base and fails there.
package vbgroup

import (
	"net/http"
	"net/http/httptest"
	"testing"

	echo "github.com/labstack/echo/v5"
)

func newStrict() *echo.Echo {
	return echo.NewWithConfig(echo.Config{
		Router: echo.NewRouter(echo.RouterConfig{AllowOverwritingRoute: false}),
	})
}

func TestVBGroupUseTwiceWithOverwritingDisallowed(t *testing.T) {
	e := newStrict()
	g := e.Group("/api")

	mw1, mw2 := false, false
	g.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		mw1 = true
		return func(c *echo.Context) error { return next(c) }
	})
	// At base this second Use panics: the implicit RouteNotFound routes are
	// re-registered and the router refuses to overwrite them.
	g.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		mw2 = true
		return func(c *echo.Context) error { return next(c) }
	})

	g.GET("/test", func(c *echo.Context) error { return c.String(http.StatusTeapot, "OK") })

	req := httptest.NewRequest(http.MethodGet, "/api/test", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusTeapot || !mw1 || !mw2 {
		t.Fatalf("both middlewares must apply: code=%d mw1=%v mw2=%v", rec.Code, mw1, mw2)
	}
}

func TestVBGroupCreatedWithMiddlewareCanUseAgain(t *testing.T) {
	e := newStrict()
	mw1 := false
	g := e.Group("/api", func(next echo.HandlerFunc) echo.HandlerFunc {
		mw1 = true
		return func(c *echo.Context) error { return next(c) }
	})
	// Creating the group with middleware already registered the implicit
	// not-found routes; a later Use must be able to replace them.
	mw2 := false
	g.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		mw2 = true
		return func(c *echo.Context) error { return next(c) }
	})
	g.GET("/x", func(c *echo.Context) error { return c.String(http.StatusOK, "x") })

	req := httptest.NewRequest(http.MethodGet, "/api/x", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK || !mw1 || !mw2 {
		t.Fatalf("both middlewares must apply: code=%d mw1=%v mw2=%v", rec.Code, mw1, mw2)
	}
}

func TestVBSubgroupUseWithOverwritingDisallowed(t *testing.T) {
	e := newStrict()
	parent := e.Group("/api")
	parent.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c *echo.Context) error { return next(c) }
	})
	child := parent.Group("/v1")
	child.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c *echo.Context) error { return next(c) }
	})
	child.GET("/ok", func(c *echo.Context) error { return c.String(http.StatusOK, "ok") })

	req := httptest.NewRequest(http.MethodGet, "/api/v1/ok", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK {
		t.Fatalf("nested groups with Use must work: %d", rec.Code)
	}
}
