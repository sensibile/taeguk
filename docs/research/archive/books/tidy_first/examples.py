"""학습용 독립 예제. 원서 코드의 번역 또는 복제본이 아니다."""


def fee_before(weight, remote):
    if weight > 0:
        if weight <= 20:
            fee = 3000 + weight * 200
            if remote:
                fee += 4000
            return fee
    return None


def fee_after(weight, remote):
    if weight <= 0:
        return None
    if weight > 20:
        return None

    base_fee = 3000
    per_kg_fee = 200
    remote_fee = 4000
    shipping_fee = base_fee + weight * per_kg_fee
    if remote:
        shipping_fee += remote_fee
    return shipping_fee


def notify_before(active, events):
    if active:
        events.append("notify")
    events.append("audit")


def notify_wrong(active, events):
    if not active:
        return
    events.append("notify")
    events.append("audit")


def notify_after(active, events):
    # The audit event is required for both active and inactive users.
    if active:
        events.append("notify")
    events.append("audit")


def run_checks():
    checks = 0
    cases = [(-1, False, None), (0, True, None), (1, False, 3200),
             (1, True, 7200), (20, False, 7000), (20, True, 11000),
             (21, False, None)]
    for weight, remote, expected in cases:
        assert fee_before(weight, remote) == expected
        assert fee_after(weight, remote) == expected
        checks += 2
    for active, expected in [(False, ["audit"]), (True, ["notify", "audit"])]:
        before, after = [], []
        notify_before(active, before)
        notify_after(active, after)
        assert before == after == expected
        checks += 1
    wrong = []
    notify_wrong(False, wrong)
    assert wrong != ["audit"]
    checks += 1
    print(f"{checks} checks passed; incorrect guard clause detected")


if __name__ == "__main__":
    run_checks()
