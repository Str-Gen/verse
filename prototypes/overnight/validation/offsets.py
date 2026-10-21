"""Reference offset conversions; scalar offsets are Python string indices."""

def scalar_to_utf16(text, offset):
    if type(offset) is not int or not 0 <= offset <= len(text):
        raise ValueError('Scalar offset outside text')
    return len(text[:offset].encode('utf-16-le')) // 2


def utf16_to_scalar(text, offset):
    if type(offset) is not int or offset < 0:
        raise ValueError('Invalid UTF-16 offset')
    units = 0
    for index, char in enumerate(text):
        if units == offset:
            return index
        units += 2 if ord(char) > 0xffff else 1
        if units > offset:
            raise ValueError('UTF-16 offset splits a surrogate pair')
    if units == offset:
        return len(text)
    raise ValueError('UTF-16 offset outside text')


# Hand-written boundaries. The combining acute accent is a separate scalar.
VECTORS = [{'text':'A😀B𝄞é', 'scalar_boundaries':[0,1,2,3,4,5,6],
            'utf16_boundaries':[0,1,3,4,6,7,8], 'invalid_utf16':[2,5]}]
