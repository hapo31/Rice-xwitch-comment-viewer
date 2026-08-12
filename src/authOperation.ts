export class AuthOperationController {
  private generation = 0;
  private pollInFlight = false;

  begin(): number {
    this.generation += 1;
    this.pollInFlight = false;
    return this.generation;
  }

  isCurrent(generation: number): boolean {
    return generation === this.generation;
  }

  tryBeginPoll(): number | undefined {
    if (this.pollInFlight) return undefined;
    this.pollInFlight = true;
    return this.generation;
  }

  finishPoll(generation: number) {
    if (this.isCurrent(generation)) this.pollInFlight = false;
  }
}
