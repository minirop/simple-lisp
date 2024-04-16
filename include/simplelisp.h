#include <string>
#include <variant>
#include <iostream>
#include <vector>
#include <functional>
#include <memory>
#include <type_traits>

using namespace std::string_literals;

struct ValueImpl;
class SimpleListObject;

#define IS_NULL(x) std::holds_alternative<std::monostate>(x)
#define IS_INT(x) std::holds_alternative<int>(x)
#define IS_FLOAT(x) std::holds_alternative<float>(x)
#define IS_STR(x) std::holds_alternative<std::string>(x)
#define IS_BOOL(x) std::holds_alternative<bool>(x)
#define IS_VEC(x) std::holds_alternative<std::vector<Value>>(x)
#define IS_FUNC(x) std::holds_alternative<Value::Function>(x)
#define IS_INSTANCE(x) std::holds_alternative<std::shared_ptr<SimpleListObject>>(x)

#define AS_INT(x) std::get<int>(x)
#define AS_FLOAT(x) std::get<float>(x)
#define AS_STR(x) std::get<std::string>(x)
#define AS_BOOL(x) std::get<bool>(x)
#define AS_VEC(x) std::get<std::vector<Value>>(x)

class Value
{
public:
    using Function = std::function<Value(std::vector<Value>)>;

    Value() : inner {} {}
    Value(int i) : inner { i } {}
    Value(float f) : inner { f } {}
    Value(std::string s) : inner { s } {}
    Value(std::vector<Value> v) : inner { v } {}
    Value(std::string name, Function&& f) : name { name }, inner { f } {}
    Value(Function&& f) : inner { f } {}
    Value(SimpleListObject* obj) : inner { std::shared_ptr<SimpleListObject>(obj) } {}
    Value(const Value& v) = default;
    Value(Value&& v) = default;

    Value& operator=(const Value&) = default;
    Value& operator=(Value&&) = default;

    Value operator()(std::vector<Value> args)
    {
        return as_func()(args);
    }

    Function& as_func()
    {
        if (IS_FUNC(inner))
        {
            return std::get<Function>(inner);
        }

        std::cerr << "value is not a function but " << get_type() << '\n';
        std::exit(1);
    }

    SimpleListObject* as_instance() const
    {
        if (is_instance())
        {
            return std::get<std::shared_ptr<SimpleListObject>>(inner).get();
        }

        std::cerr << "value is not an instance but " << get_type() << '\n';
        std::exit(1);
    }

    std::string as_string() const
    {
        if (IS_STR(inner))
        {
            return AS_STR(inner);
        }
        
        std::cerr << "value is not an string nor convertible to a string but " << get_type() << '\n';
        std::exit(1);
    }

    bool is_instance() const
    {
        return IS_INSTANCE(inner);
    }

    std::string get_type() const
    {
        if (IS_NULL(inner))
        {
            return "NULL";
        }
        else if (IS_INT(inner))
        {
            return "int";
        }
        else if (IS_FLOAT(inner))
        {
            return "float";
        }
        else if (IS_STR(inner))
        {
            return "string";
        }
        else if (IS_VEC(inner))
        {
            std::string str = "[ ";
            for (auto & item : AS_VEC(inner))
            {
                str += item.get_type() + " ";
            }
            str += "]";
            return str;
        }
        else if (IS_FUNC(inner))
        {
            return "function";
        }
        else if (IS_INSTANCE(inner))
        {
            return "instance";
        }

        return "unknown";
    }

private:
    friend std::ostream& operator<<(std::ostream& os, const Value& obj);
    friend Value operator+(Value lhs, const Value& rhs);
    friend Value operator-(Value lhs, const Value& rhs);
    friend Value operator*(Value lhs, const Value& rhs);
    friend Value operator/(Value lhs, const Value& rhs);
    friend bool operator< (const Value& lhs, const Value& rhs);
    friend bool operator==(const Value& lhs, const Value& rhs);

    std::string name;
    std::variant<std::monostate, int, float, std::string, std::vector<Value>, Function, std::shared_ptr<SimpleListObject>> inner;
};

std::ostream& operator<<(std::ostream& os, const Value& obj)
{
    auto & inner = obj.inner;
    if (IS_NULL(inner))
    {
        os << "NULL";
    }
    else if (IS_INT(inner))
    {
        os << AS_INT(inner);
    }
    else if (IS_FLOAT(inner))
    {
        os << AS_FLOAT(inner);
    }
    else if (IS_STR(inner))
    {
        os << AS_STR(inner);
    }
    else if (IS_VEC(inner))
    {
        os << "[ ";
        for (auto & item : AS_VEC(inner))
        {
            os << item << " ";
        }
        os << "]";
    }
    else if (IS_FUNC(inner))
    {
        os << "<lambda#1>";
    }

    return os;
}

inline bool operator< (const Value& lhs, const Value& rhs) {
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;

    if (IS_INT(l))
    {
        if (IS_INT(r))
        {
            return AS_INT(l) < AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return AS_INT(l) < AS_FLOAT(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        if (IS_INT(r))
        {
            return AS_FLOAT(l) < AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return AS_FLOAT(l) < AS_FLOAT(r);
        }
    }
    else if (IS_STR(l))
    {
        if (IS_STR(r))
        {
            return AS_STR(l) < AS_STR(r);
        }
    }

    return false;
}

inline bool operator> (const Value& lhs, const Value& rhs) { return rhs < lhs; }
inline bool operator<=(const Value& lhs, const Value& rhs) { return !(lhs > rhs); }
inline bool operator>=(const Value& lhs, const Value& rhs) { return !(lhs < rhs); }
inline bool operator==(const Value& lhs, const Value& rhs)
{
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;

    if (IS_INT(l))
    {
        auto li = AS_INT(l);
        if (IS_INT(r))
        {
            return li == AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return li == AS_FLOAT(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        auto lf = AS_FLOAT(l);
        if (IS_INT(r))
        {
            return lf + AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return lf + AS_FLOAT(r);
        }
    }
    else if (IS_STR(l))
    {
        if (IS_STR(r))
        {
            return AS_STR(l) == AS_STR(r);
        }
    }

    return false;
}

inline bool operator!=(const Value& lhs, const Value& rhs) { return !(lhs == rhs); }

Value operator+(Value lhs, const Value& rhs)
{
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;

    if (IS_INT(l))
    {
        auto li = AS_INT(l);
        if (IS_INT(r))
        {
            return li + AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return li + AS_FLOAT(r);
        }
        else if (IS_STR(r))
        {
            return std::to_string(li) + AS_STR(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        auto lf = AS_FLOAT(l);
        if (IS_INT(r))
        {
            return lf + AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return lf + AS_FLOAT(r);
        }
        else if (IS_STR(r))
        {
            return std::to_string(lf) + AS_STR(r);
        }
    }
    else if (IS_STR(l))
    {
        auto ls = AS_STR(l);
        if (IS_INT(r))
        {
            return ls + std::to_string(AS_INT(r));
        }
        else if (IS_FLOAT(r))
        {
            return ls + std::to_string(AS_FLOAT(r));
        }
        else if (IS_STR(r))
        {
            return ls + AS_STR(r);
        }
    }

    return Value();
}

Value operator-(Value lhs, const Value& rhs)
{
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;

    if (IS_INT(l))
    {
        auto li = AS_INT(l);
        if (IS_INT(r))
        {
            return li - AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return li - AS_FLOAT(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        auto lf = AS_FLOAT(l);
        if (IS_INT(r))
        {
            return lf - AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return lf - AS_FLOAT(r);
        }
    }

    return Value();
}

Value operator*(Value lhs, const Value& rhs)
{
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;
    
    if (IS_INT(l))
    {
        auto li = AS_INT(l);
        if (IS_INT(r))
        {
            return li * AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return li * AS_FLOAT(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        auto lf = AS_FLOAT(l);
        if (IS_INT(r))
        {
            return lf * AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return lf * AS_FLOAT(r);
        }
    }
    else if (IS_STR(l))
    {
        if (IS_INT(r))
        {
            std::string tmp;
            auto sss = AS_STR(l);
            for (int i = 0; i < AS_INT(r); ++i)
            {
                tmp += sss;
            }
            return tmp;
        }
    }

    return Value();
}

Value operator/(Value lhs, const Value& rhs)
{
    const auto & l = lhs.inner;
    const auto & r = rhs.inner;

    if (IS_INT(l))
    {
        auto li = AS_INT(l);
        if (IS_INT(r))
        {
            return li / AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return li / AS_FLOAT(r);
        }
    }
    else if (IS_FLOAT(l))
    {
        auto lf = AS_FLOAT(l);
        if (IS_INT(r))
        {
            return lf / AS_INT(r);
        }
        else if (IS_FLOAT(r))
        {
            return lf / AS_FLOAT(r);
        }
    }

    return Value();
}

inline Value func_print(Value arg0)
{
    std::cout << arg0 << '\n';
    return Value();
}

inline Value func_write(Value arg0)
{
    std::cout << arg0;
    return Value();
}

inline Value func_print(Value arg0, Value arg1)
{
    std::cout << arg0 << arg1 << '\n';
    return Value();
}

inline Value func_write(Value arg0, Value arg1)
{
    std::cout << arg0 << arg1;
    return Value();
}
