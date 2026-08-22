// Hidden fail-to-pass tests: RFC 9457 Problem Details HTTP error handler.
// Test-only sub-package: references the new API, so it does not compile at
// the base commit while the pass-to-pass guards still do.
package vbproblem

import (
	"encoding/json"
	"errors"
	"net/http"
	"net/http/httptest"
	"testing"

	echo "github.com/labstack/echo/v5"
)

func serve(t *testing.T, exposeError bool, method string, err error) *httptest.ResponseRecorder {
	t.Helper()
	e := echo.New()
	e.Any("/path", func(c *echo.Context) error { return err })
	e.HTTPErrorHandler = echo.ProblemDetailsHTTPErrorHandler(exposeError)
	req := httptest.NewRequest(method, "/path", nil)
	rec := httptest.NewRecorder()
	e.ServeHTTP(rec, req)
	return rec
}

func decode(t *testing.T, rec *httptest.ResponseRecorder) map[string]any {
	t.Helper()
	var body map[string]any
	if err := json.Unmarshal(rec.Body.Bytes(), &body); err != nil {
		t.Fatalf("response body is not JSON: %v (%q)", err, rec.Body.String())
	}
	return body
}

func TestVBPlainErrorNotExposed(t *testing.T) {
	rec := serve(t, false, http.MethodGet, errors.New("secret detail"))
	if rec.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d, want 500", rec.Code)
	}
	if ct := rec.Header().Get("Content-Type"); ct != "application/problem+json" {
		t.Fatalf("content type = %q, want application/problem+json", ct)
	}
	body := decode(t, rec)
	if body["type"] != "about:blank" || body["status"] != float64(500) {
		t.Fatalf("unexpected problem body: %v", body)
	}
	if _, ok := body["detail"]; ok {
		t.Fatalf("detail must not be exposed when exposeError=false: %v", body)
	}
}

func TestVBPlainErrorExposed(t *testing.T) {
	rec := serve(t, true, http.MethodGet, errors.New("boom goes the dynamite"))
	body := decode(t, rec)
	if body["detail"] != "boom goes the dynamite" {
		t.Fatalf("detail = %v, want the error text when exposeError=true", body["detail"])
	}
}

func TestVBHTTPErrorMapped(t *testing.T) {
	rec := serve(t, false, http.MethodGet, &echo.HTTPError{Code: http.StatusTeapot, Message: "my_error"})
	if rec.Code != http.StatusTeapot {
		t.Fatalf("status = %d, want 418", rec.Code)
	}
	body := decode(t, rec)
	if body["status"] != float64(418) || body["detail"] != "my_error" {
		t.Fatalf("unexpected problem body: %v", body)
	}
}

func TestVBProblemErrorUsedAsIs(t *testing.T) {
	pe := &echo.ProblemError{
		Type:     "https://example.com/probs/out-of-credit",
		Title:    "You do not have enough credit.",
		Status:   http.StatusForbidden,
		Detail:   "Your current balance is 30, but that costs 50.",
		Instance: "/account/12345/msgs/abc",
	}
	rec := serve(t, false, http.MethodGet, pe)
	if rec.Code != http.StatusForbidden {
		t.Fatalf("status = %d, want 403", rec.Code)
	}
	body := decode(t, rec)
	if body["type"] != pe.Type || body["title"] != pe.Title || body["instance"] != pe.Instance {
		t.Fatalf("problem fields not passed through: %v", body)
	}
}

func TestVBProblemErrorZeroDefaults(t *testing.T) {
	rec := serve(t, false, http.MethodGet, &echo.ProblemError{})
	body := decode(t, rec)
	if rec.Code != http.StatusInternalServerError || body["type"] != "about:blank" {
		t.Fatalf("zero-value ProblemError must default to 500/about:blank: %d %v", rec.Code, body)
	}
	if body["title"] != http.StatusText(http.StatusInternalServerError) {
		t.Fatalf("title must default to the status text: %v", body["title"])
	}
}

type customProblemErrorer struct{ pe *echo.ProblemError }

func (ce *customProblemErrorer) Error() string                  { return "custom" }
func (ce *customProblemErrorer) ProblemError() *echo.ProblemError { return ce.pe }

func TestVBProblemErrorerConversion(t *testing.T) {
	err := &customProblemErrorer{pe: &echo.ProblemError{Status: http.StatusConflict, Detail: "already exists"}}
	rec := serve(t, false, http.MethodGet, err)
	if rec.Code != http.StatusConflict {
		t.Fatalf("status = %d, want 409", rec.Code)
	}
	body := decode(t, rec)
	if body["detail"] != "already exists" {
		t.Fatalf("ProblemErrorer conversion lost detail: %v", body)
	}
}

func TestVBHeadRequestNoBody(t *testing.T) {
	rec := serve(t, true, http.MethodHead, errors.New("whatever"))
	if rec.Code != http.StatusInternalServerError {
		t.Fatalf("status = %d, want 500", rec.Code)
	}
	if rec.Body.Len() != 0 {
		t.Fatalf("HEAD response must have no body, got %q", rec.Body.String())
	}
}
