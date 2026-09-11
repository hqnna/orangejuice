//
// The three arrays: fixed, view, and resizable (**L§3.6**).
//

#import "Basic";

sum :: (numbers: [] int) -> int {
    total := 0;
    for numbers  total += it;
    return total;
}

main :: () {
    // A fixed array carries its length in its type, so nothing is stored
    // beside the elements.
    fixed: [4] int;
    for * fixed  it.* = it_index * it_index;
    print("% (% bytes)\n", fixed, size_of(type_of(fixed)));

    // A view is a count and a pointer at someone else's elements. A fixed
    // array converts to one, which is why `sum` takes every kind of array.
    print("sum = %\n", sum(fixed));

    // A slice names part of one without copying: `sum` is handed a view of
    // two elements over storage the fixed array still owns.
    middle: [] int = fixed;
    middle.data += 1;
    middle.count = 2;
    print("middle two: % (%)\n", middle, sum(middle));

    // A resizable array owns its elements and grows through the context
    // allocator.
    growing: [..] string;
    defer array_free(growing);
    array_add(*growing, "first");
    array_add(*growing, "second");
    array_add(*growing, "third");
    print("% items: %\n", growing.count, growing);

    // Removing by index is unordered by default: the last element is moved
    // into the hole, which is why the order changes here.
    array_unordered_remove_by_index(*growing, 0);
    print("after removing the first: %\n", growing);

    // A literal array needs no length written.
    primes := int.[2, 3, 5, 7, 11];
    print("% primes, largest %\n", primes.count, primes[primes.count - 1]);
}
