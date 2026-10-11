// Original minimal probe of language claims, not copied book code.
public class LegacyLanguageProbe {
    static final class DeclaredFailure extends Exception {}
    static double meanOfEmpty() throws DeclaredFailure {
        double total = 0.0;
        int count = 0;
        return total / count;
    }
    public static void main(String[] args) throws Exception {
        double result = meanOfEmpty();
        if (!Double.isNaN(result)) throw new AssertionError("expected NaN");
        System.out.println("declared-but-unthrown checked exception: compiled");
        System.out.println("double zero / int zero: NaN; no ArithmeticException");
    }
}
