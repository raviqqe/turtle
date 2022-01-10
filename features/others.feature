Feature: Others
  Scenario: Build nothing
    Given a file named "build.ninja" with:
    """
    """
    When I successfully run `turtle`
    Then the exit status should be 0

  Scenario: Do not rebuild an up-to-date output
    Given a file named "build.ninja" with:
    """
    rule cp
      command = echo hello && cp $in $out

    build foo: cp bar

    """
    And a file named "bar" with ""
    When I successfully run `turtle`
    And I successfully run `turtle`
    Then the stderr should contain exactly "hello"

  Scenario: Rebuild a stale output
    Given a file named "build.ninja" with:
    """
    rule cp
      command = echo hello && cp $in $out

    build foo: cp bar

    """
    And a file named "bar" with ""
    When I successfully run `turtle`
    And I successfully run `touch bar`
    And I successfully run `turtle`
    Then the stderr should contain exactly:
    """
    hello
    hello
    """

  Scenario: Chain rebuilds
    Given a file named "build.ninja" with:
    """
    rule cp
      command = echo hello && cp $in $out

    build bar: cp baz
    build foo: cp bar

    """
    And a file named "baz" with ""
    When I successfully run `turtle`
    And I successfully run `touch baz`
    And I successfully run `turtle`
    Then the stderr should contain exactly:
    """
    hello
    hello
    hello
    hello
    """

  Scenario: Use a custom build file location
    Given a file named "foo.ninja" with:
    """
    rule echo
      command = echo hello

    build foo: echo

    """
    When I successfully run `turtle -f foo.ninja`
    Then the stderr should contain exactly "hello"

  Scenario: Rerun a failed rule
    Given a file named "build.ninja" with:
    """
    rule fail
      command = exit 1

    build foo: fail

    """
    When I run `turtle`
    And I run `turtle`
    Then the exit status should not be 0
    And the stderr should contain exactly:
    """
    hello
    hello
    """
