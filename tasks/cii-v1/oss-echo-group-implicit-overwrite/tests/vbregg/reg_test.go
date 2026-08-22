// Hidden pass-to-pass guards: explicit-route overwrite protection and group
// middleware behavior unchanged. Compiles and passes at base.
package vbregg

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

func TestVBExplicitRouteStillProtected(t *testing.T) {
	defer func() {
		if recover() == nil {
			t.Fatal("re-registering an explicit route with overwriting disallowed must still fail")
		}
	}()
	e := newStrict()
	e.GET("/dup", func(c *echo.Context) error { return nil })
	e.GET("/dup", func(c *echo.Context) error { return nil })
}

func TestVBGroupMiddlewareRunsOn404InPrefix(t *testing.T) {
	e := echo.New()
	g := e.Group("/api")
	hit := false
	g.Use(func(next echo.HandlerFunc) echo.HandlerFunc {
		return func(c *echo.Context) error { hit = true; return next(c) }
	})
	g.GET("/known", func(c *echo.Context) error { return c.String(http.StatusOK, "ok") })

	req := httptest.NewRequest(http.MethodGet, "/api/unknown", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusNotFound || !hit {
		t.Fatalf("group middleware must run for 404 inside the prefix: %d hit=%v", rec.Code, hit)
	}
}

func TestVBGroupRoutingWorks(t *testing.T) {
	e := echo.New()
	g := e.Group("/api")
	g.GET("/thing/:id", func(c *echo.Context) error { return c.String(http.StatusOK, c.Param("id")) })

	req := httptest.NewRequest(http.MethodGet, "/api/thing/9", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	if rec.Code != http.StatusOK || rec.Body.String() != "9" {
		t.Fatalf("group routing broken: %d %q", rec.Code, rec.Body.String())
	}
}
