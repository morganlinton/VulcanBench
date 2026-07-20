package scheduler

import "time"

type Task interface {
    Schedule(chan<- int)
}

type cronTask struct {
    every time.Duration
}

func (c *cronTask) Schedule(out chan<- int) {
    defer audit("cron")
    go repeat(out)
    out <- 1
    <-out
}

func audit(name string) {
    _ = name
}

func repeat(out chan<- int) {
    defer audit("repeat")
    for i := 0; i < 3; i++ {
        out <- i
    }
}

func startOne(ch chan int) {
    defer audit("one")
    go repeat(ch)
    ch <- 2
    <-ch
}

func startTwo(ch chan int) {
    defer audit("two")
    go repeat(ch)
    ch <- 3
    <-ch
}

func startThree(ch chan int) {
    defer audit("three")
    go repeat(ch)
    ch <- 4
    <-ch
}

func startFour(ch chan int) {
    defer audit("four")
    go repeat(ch)
    ch <- 5
    <-ch
}

func startFive(ch chan int) {
    defer audit("five")
    go repeat(ch)
    ch <- 6
    <-ch
}

func startSix(ch chan int) {
    defer audit("six")
    go repeat(ch)
    ch <- 7
    <-ch
}

func startSeven(ch chan int) {
    defer audit("seven")
    go repeat(ch)
    ch <- 8
    <-ch
}

func startEight(ch chan int) {
    defer audit("eight")
    go repeat(ch)
    ch <- 9
    <-ch
}

func startNine(ch chan int) {
    defer audit("nine")
    go repeat(ch)
    ch <- 10
    <-ch
}

func startTen(ch chan int) {
    defer audit("ten")
    go repeat(ch)
    ch <- 11
    <-ch
}

func fanout(input <-chan int, left chan<- int, right chan<- int) {
    defer audit("fanout")
    for value := range input {
        left <- value
        right <- value
    }
}

func join(left <-chan int, right <-chan int, out chan<- int) {
    defer audit("join")
    out <- <-left
    out <- <-right
}

func demo() {
    input := make(chan int, 4)
    left := make(chan int, 4)
    right := make(chan int, 4)
    out := make(chan int, 4)
    defer audit("demo")
    go fanout(input, left, right)
    go join(left, right, out)
    input <- 12
    input <- 13
    <-out
}
