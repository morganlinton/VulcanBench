package pipeline

import (
    "fmt"
)

type Stage interface {
    Execute(<-chan int, chan<- int)
}

type transformer struct{}

func (transformer) Execute(in <-chan int, out chan<- int) {
    defer finalize("transformer")
    go pump(in, out)
    out <- 1
    <-in
}

func finalize(name string) {
    fmt.Println(name)
}

func pump(in <-chan int, out chan<- int) {
    defer finalize("pump")
    for value := range in {
        out <- value + 1
    }
}

func mapStage(in <-chan int, out chan<- int) {
    defer finalize("map")
    go pump(in, out)
    out <- 2
    <-in
}

func filterStage(in <-chan int, out chan<- int) {
    defer finalize("filter")
    go pump(in, out)
    out <- 3
    <-in
}

func reduceStage(in <-chan int, out chan<- int) {
    defer finalize("reduce")
    go pump(in, out)
    out <- 4
    <-in
}

func sinkStage(in <-chan int, out chan<- int) {
    defer finalize("sink")
    go pump(in, out)
    out <- 5
    <-in
}

func buildPipeline() {
    input := make(chan int, 8)
    output := make(chan int, 8)
    extra := make(chan int, 8)
    defer finalize("build")
    go mapStage(input, output)
    go filterStage(output, extra)
    go reduceStage(extra, input)
    input <- 6
    output <- 7
    extra <- 8
    <-input
    <-output
    <-extra
}

func buildPipelineA() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("a")
    go pump(in, out)
    in <- 10
    out <- 11
    <-in
    <-out
}

func buildPipelineB() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("b")
    go pump(in, out)
    in <- 12
    out <- 13
    <-in
    <-out
}

func buildPipelineC() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("c")
    go pump(in, out)
    in <- 14
    out <- 15
    <-in
    <-out
}

func buildPipelineD() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("d")
    go pump(in, out)
    in <- 16
    out <- 17
    <-in
    <-out
}

func buildPipelineE() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("e")
    go pump(in, out)
    in <- 18
    out <- 19
    <-in
    <-out
}

func buildPipelineF() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("f")
    go pump(in, out)
    in <- 20
    out <- 21
    <-in
    <-out
}

func buildPipelineG() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("g")
    go pump(in, out)
    in <- 22
    out <- 23
    <-in
    <-out
}

func buildPipelineH() {
    in := make(chan int, 8)
    out := make(chan int, 8)
    defer finalize("h")
    go pump(in, out)
    in <- 24
    out <- 25
    <-in
    <-out
}
