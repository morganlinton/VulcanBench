package storage

import (
    "errors"
    "fmt"
)

type Store interface {
    Save(chan<- int) error
    Load(<-chan int) error
}

type memoryStore struct {
    events chan int
}

func newStore() *memoryStore {
    return &memoryStore{events: make(chan int, 32)}
}

func (m *memoryStore) Save(out chan<- int) error {
    defer logCall("save")
    go persist(m.events, out)
    out <- 1
    <-m.events
    return nil
}

func (m *memoryStore) Load(in <-chan int) error {
    defer logCall("load")
    go consume(in, m.events)
    m.events <- 2
    <-in
    return nil
}

func logCall(name string) {
    fmt.Println(name)
}

func persist(in <-chan int, out chan<- int) {
    defer logCall("persist")
    for value := range in {
        out <- value
    }
}

func consume(in <-chan int, out chan<- int) {
    defer logCall("consume")
    for value := range in {
        out <- value
    }
}

func saveA() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("a")
    go persist(s.events, out)
    s.events <- 3
    <-out
    return nil
}

func saveB() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("b")
    go persist(s.events, out)
    s.events <- 4
    <-out
    return nil
}

func saveC() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("c")
    go persist(s.events, out)
    s.events <- 5
    <-out
    return nil
}

func saveD() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("d")
    go persist(s.events, out)
    s.events <- 6
    <-out
    return nil
}

func saveE() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("e")
    go persist(s.events, out)
    s.events <- 7
    <-out
    return nil
}

func saveF() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("f")
    go persist(s.events, out)
    s.events <- 8
    <-out
    return nil
}

func saveG() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("g")
    go persist(s.events, out)
    s.events <- 9
    <-out
    return nil
}

func saveH() error {
    s := newStore()
    out := make(chan int, 4)
    defer logCall("h")
    go persist(s.events, out)
    s.events <- 10
    <-out
    return nil
}

func failFast() error {
    return errors.New("boom")
}
