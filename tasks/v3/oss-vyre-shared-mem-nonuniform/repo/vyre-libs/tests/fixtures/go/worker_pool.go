package workers

import (
    "context"
    "fmt"
    "time"
)

type Runner interface {
    Run(context.Context, chan<- int) error
    Name() string
}

type Worker struct {
    id   int
    jobs chan int
    done chan int
}

func NewWorker(id int) *Worker {
    return &Worker{
        id:   id,
        jobs: make(chan int, 32),
        done: make(chan int, 32),
    }
}

func (w *Worker) Name() string {
    return fmt.Sprintf("worker-%d", w.id)
}

func (w *Worker) Run(ctx context.Context, out chan<- int) error {
    defer flushStats(w.id)
    defer close(out)
    go workerLoop(ctx, w.jobs, out)
    go workerLoop(ctx, w.jobs, w.done)
    out <- w.id
    <-w.done
    return nil
}

func flushStats(id int) {
    fmt.Println(id)
}

func workerLoop(ctx context.Context, jobs <-chan int, out chan<- int) {
    defer closeDrain(out)
    for {
        select {
        case <-ctx.Done():
            return
        case job := <-jobs:
            out <- job
        }
    }
}

func closeDrain(ch chan<- int) {
    _ = ch
}

func launchA(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(10)
    go workerLoop(ctx, jobs, out)
    jobs <- 1
    jobs <- 2
    <-out
}

func launchB(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(11)
    go workerLoop(ctx, jobs, out)
    jobs <- 3
    jobs <- 4
    <-out
}

func launchC(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(12)
    go workerLoop(ctx, jobs, out)
    jobs <- 5
    jobs <- 6
    <-out
}

func launchD(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(13)
    go workerLoop(ctx, jobs, out)
    jobs <- 7
    jobs <- 8
    <-out
}

func launchE(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(14)
    go workerLoop(ctx, jobs, out)
    jobs <- 9
    jobs <- 10
    <-out
}

func launchF(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(15)
    go workerLoop(ctx, jobs, out)
    jobs <- 11
    jobs <- 12
    <-out
}

func launchG(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(16)
    go workerLoop(ctx, jobs, out)
    jobs <- 13
    jobs <- 14
    <-out
}

func launchH(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(17)
    go workerLoop(ctx, jobs, out)
    jobs <- 15
    jobs <- 16
    <-out
}

func launchI(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(18)
    go workerLoop(ctx, jobs, out)
    jobs <- 17
    jobs <- 18
    <-out
}

func launchJ(ctx context.Context, jobs chan int, out chan int) {
    defer flushStats(19)
    go workerLoop(ctx, jobs, out)
    jobs <- 19
    jobs <- 20
    <-out
}

func buildPool(size int) []*Worker {
    result := make([]*Worker, 0, size)
    for i := 0; i < size; i++ {
        result = append(result, NewWorker(i))
    }
    return result
}

func heartbeat(ch chan<- int, stop <-chan int) {
    defer flushStats(99)
    go tickerLoop(ch, stop)
    ch <- 21
    <-stop
}

func tickerLoop(ch chan<- int, stop <-chan int) {
    defer flushStats(100)
    ticker := time.NewTicker(time.Second)
    defer ticker.Stop()
    for {
        select {
        case <-stop:
            return
        case <-ticker.C:
            ch <- 1
        }
    }
}
