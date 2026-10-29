// @author Cursor

package main

import (
	"net/http/httptest"
	"testing"
	"time"
)

func TestEmailAndCodeNormalization(t *testing.T) {
	if normalizeEmail(" Owner@Example.com ") != "owner@example.com" {
		t.Fatal("email")
	}
	if !validEmail("owner@example.com") || validEmail("nope") {
		t.Fatal("valid")
	}
	if normalizeCode("abcd-efgh") != "ABCDEFGH" {
		t.Fatal("code")
	}
}

func TestRequestLangUsesTheFirstTag(t *testing.T) {
	ja := httptest.NewRequest("GET", "/", nil)
	ja.Header.Set("Accept-Language", "ja-JP,en;q=0.8")
	if requestLang(ja) != langJa {
		t.Fatal("ja")
	}
	en := httptest.NewRequest("GET", "/", nil)
	en.Header.Set("Accept-Language", "en-US,ja;q=0.9")
	if requestLang(en) != langEn || texts(langJa).BadLogin != "メールアドレスまたはパスワードが違います" {
		t.Fatal("en")
	}
}

func TestStaleAfterOneHour(t *testing.T) {
	if stale(time.Now()) {
		t.Fatal("fresh")
	}
	if !stale(time.Now().Add(-2 * time.Hour)) {
		t.Fatal("stale")
	}
}
