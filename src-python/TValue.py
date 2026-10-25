class TValue():
    def __init__(self) -> None:
        self.value = None
        pass

class TNumber(TValue):
    def __init__(self, value) -> None:
        self.value = value

    def __repr__(self) -> str:
        return str(self.value)

class TString(TValue):
    def __init__(self, value: str) -> None:
        self.value = value

    def __repr__(self) -> str:
        return str(self.value)

