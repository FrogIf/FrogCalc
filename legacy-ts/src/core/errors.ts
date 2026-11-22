export class CalcError extends Error {
  readonly pos?: number
  readonly code?: string

  constructor(message: string, pos?: number, code?: string) {
    super(message)
    this.name = 'CalcError'
    this.pos = pos
    this.code = code
  }
}
